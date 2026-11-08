//! In-app updates from GitHub Releases, on top of `tauri-plugin-updater`.
//!
//! The plugin reads `latest.json` from the latest release (the endpoint and
//! the public key are in `tauri.conf.json`) and refuses any download whose
//! minisign signature does not match the key. This module adds the app's
//! policy: a quiet background check (15 s after launch, then every 6 hours,
//! only when `auto_check_updates` is on), a check the user asks for, and an
//! install that runs only from the user's Update now click. Progress goes to
//! every window as `"update"` events carrying a [`Phase`]: downloading, then
//! verifying (the signature), then installing, then restarting.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_updater::{Update, UpdaterExt};

/// The event every window listens to.
pub const EVENT: &str = "update";
/// Says which version the user put off with Later (`null` clears it).
pub const DISMISSED_EVENT: &str = "update-dismissed";

const FIRST_CHECK: Duration = Duration::from_secs(15);
const CHECK_EVERY: Duration = Duration::from_secs(6 * 60 * 60);

const NOT_VERIFIED: &str = "Update could not be verified";
const UNREACHABLE: &str = "Could not reach GitHub";
const MISSING: &str = "The update file is missing from the release";
/// Progress for a download of unknown size is sent once per this many bytes.
const STEP_WITHOUT_SIZE: u64 = 1024 * 1024;

/// A newer release, as `latest.json` describes it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UpdateInfo {
    pub version: String,
    pub notes: String,
    /// `YYYY-MM-DD`, when the release says.
    pub date: Option<String>,
}

/// Where an update stands, sent as `{ "phase": "...", ... }`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "phase", rename_all = "snake_case")]
pub enum Phase {
    Available {
        version: String,
        notes: String,
        date: Option<String>,
    },
    Downloading {
        downloaded: u64,
        total: Option<u64>,
    },
    /// The download is complete; the plugin is checking its signature.
    Verifying,
    Installing,
    Restarting,
    Failed {
        error: String,
    },
    UpToDate,
}

impl Phase {
    /// Downloading, verifying, installing or restarting.
    pub fn busy(&self) -> bool {
        matches!(
            self,
            Phase::Downloading { .. } | Phase::Verifying | Phase::Installing | Phase::Restarting
        )
    }
}

impl From<UpdateInfo> for Phase {
    fn from(i: UpdateInfo) -> Self {
        Phase::Available {
            version: i.version,
            notes: i.notes,
            date: i.date,
        }
    }
}

/// True when `candidate` is a strictly higher semver than `current` (a
/// leading `v` is allowed). Anything unparsable is never newer.
pub fn is_newer(current: &str, candidate: &str) -> bool {
    let parse = |v: &str| semver::Version::parse(v.trim().trim_start_matches('v'));
    match (parse(current), parse(candidate)) {
        (Ok(current), Ok(candidate)) => candidate > current,
        _ => false,
    }
}

fn failed(error: &str) -> Phase {
    Phase::Failed {
        error: error.into(),
    }
}

/// What to tell the user about a failed check or install, from the error's
/// text: a signature problem means nothing was installed, a 404 on the
/// download means the release lacks the file the feed names, other network
/// problems mean GitHub could not be reached; anything else keeps its words.
pub fn classify_error(e: &str) -> Phase {
    let lower = e.to_lowercase();
    let has = |words: &[&str]| words.iter().any(|w| lower.contains(w));
    if has(&[
        "signature",
        "signed for",
        "minisign",
        "verif",
        "public key",
        "base64",
    ]) {
        failed(NOT_VERIFIED)
    } else if has(&["status: 404"]) {
        failed(MISSING)
    } else if has(&[
        "error sending request",
        "request failed",
        "dns",
        "connect",
        "timed out",
        "network",
        "release json",
        "status:",
    ]) {
        failed(UNREACHABLE)
    } else {
        Phase::Failed {
            error: format!("Update failed: {e}"),
        }
    }
}

/// [`classify_error`] on the plugin's error, by kind where the kind says it.
pub fn classify(e: &tauri_plugin_updater::Error) -> Phase {
    use tauri_plugin_updater::Error as E;
    match e {
        E::Minisign(_)
        | E::Base64(_)
        | E::SignatureUtf8(_)
        | E::SignedVersionMismatch { .. }
        | E::MissingSignedVersion => failed(NOT_VERIFIED),
        E::Network(text) => classify_error(text),
        E::Reqwest(_) | E::ReleaseNotFound => failed(UNREACHABLE),
        other => classify_error(&other.to_string()),
    }
}

/// Remembers whether the last background check failed, so an offline
/// machine logs one line per outage instead of one per check.
#[derive(Debug, Default)]
pub struct BackgroundLog {
    failing: bool,
}

/// What a background check does with its result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Background {
    /// `Available`, for a newer release; never anything else.
    pub emit: Option<Phase>,
    /// A line for the log, when a run of failures starts.
    pub log: Option<String>,
    /// Nothing newer is out: forget a release remembered earlier.
    pub forget: bool,
}

/// Emit `Available` for a newer release and otherwise nothing; a failure is
/// logged once per run of failures and never shown.
pub fn background_outcome(
    current: &str,
    result: Result<Option<UpdateInfo>, String>,
    log: &mut BackgroundLog,
) -> Background {
    match result {
        Err(e) => {
            let line = (!log.failing).then(|| format!("update check failed: {e}"));
            log.failing = true;
            Background {
                emit: None,
                log: line,
                forget: false,
            }
        }
        Ok(found) => {
            log.failing = false;
            let emit = found
                .filter(|i| is_newer(current, &i.version))
                .map(Phase::from);
            Background {
                forget: emit.is_none(),
                emit,
                log: None,
            }
        }
    }
}

/// Lets a download's progress through once per whole percent (once per MiB
/// when the size is unknown), so the windows are not flooded.
#[derive(Debug, Default)]
pub struct PercentGate {
    last: Option<u64>,
}

impl PercentGate {
    /// True when `downloaded` reaches a percent (or MiB) not reported yet.
    pub fn pass(&mut self, downloaded: u64, total: Option<u64>) -> bool {
        let key = match total {
            Some(t) if t > 0 => downloaded.min(t) * 100 / t,
            _ => downloaded / STEP_WITHOUT_SIZE,
        };
        if self.last == Some(key) {
            return false;
        }
        self.last = Some(key);
        true
    }
}

/// One thing that happened during an install.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Progress {
    /// Bytes so far, out of `total` when the server said.
    Chunk { downloaded: u64, total: Option<u64> },
    /// Every byte has arrived; the plugin checks the signature next.
    DownloadFinished { downloaded: u64, total: Option<u64> },
    /// The signature matched; installing starts now.
    Verified,
    /// The installer's result.
    Installed(Result<(), String>),
}

/// What to emit for a [`Progress`], and whether to relaunch now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    pub emit: Vec<Phase>,
    pub restart: bool,
}

/// The install sequence: the bar reaches 100 % before `Verifying`,
/// `Installing` follows only a matching signature, and the relaunch happens
/// only after the installer returned Ok.
pub fn next_step(p: Progress) -> Step {
    let step = |emit: Vec<Phase>| Step {
        emit,
        restart: false,
    };
    match p {
        Progress::Chunk { downloaded, total } => step(vec![Phase::Downloading {
            downloaded: total.map_or(downloaded, |t| downloaded.min(t)),
            total,
        }]),
        Progress::DownloadFinished { downloaded, total } => step(vec![
            Phase::Downloading {
                downloaded: total.unwrap_or(downloaded),
                total,
            },
            Phase::Verifying,
        ]),
        Progress::Verified => step(vec![Phase::Installing]),
        Progress::Installed(Ok(())) => Step {
            emit: vec![Phase::Restarting],
            restart: true,
        },
        Progress::Installed(Err(e)) => step(vec![classify_error(&e)]),
    }
}

/// The app's memory of the update: the last release found, the version put
/// off with Later and the install's last phase. Kept for windows opened
/// later; lost on relaunch.
#[derive(Debug, Default)]
pub struct UpdateState {
    inner: Mutex<Snapshot>,
    installing: AtomicBool,
}

/// What a window needs when it opens.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Snapshot {
    pub available: Option<UpdateInfo>,
    pub dismissed: Option<String>,
    /// The last phase sent, so a window opened mid-download shows progress.
    pub phase: Option<Phase>,
    /// True while Update now is running.
    pub installing: bool,
}

fn info_of(u: &Update) -> UpdateInfo {
    UpdateInfo {
        version: u.version.clone(),
        notes: u.body.clone().unwrap_or_default(),
        date: u
            .date
            .map(|d| format!("{:04}-{:02}-{:02}", d.year(), u8::from(d.month()), d.day())),
    }
}

fn current(app: &AppHandle) -> String {
    app.package_info().version.to_string()
}

fn with_state(app: &AppHandle, f: impl FnOnce(&mut Snapshot)) {
    if let Some(state) = app.try_state::<UpdateState>() {
        if let Ok(mut s) = state.inner.lock() {
            f(&mut s);
        }
    }
}

/// Sends `phase` to every window and remembers it; a background notice does
/// not replace an install's phase.
fn emit(app: &AppHandle, phase: &Phase) {
    with_state(app, |s| {
        if let Phase::Available {
            version,
            notes,
            date,
        } = phase
        {
            s.available = Some(UpdateInfo {
                version: version.clone(),
                notes: notes.clone(),
                date: date.clone(),
            });
        }
        if !(matches!(phase, Phase::Available { .. }) && s.phase.as_ref().is_some_and(Phase::busy))
        {
            s.phase = Some(phase.clone());
        }
    });
    let _ = app.emit(EVENT, phase);
}

/// What the app closes on a normal exit. The Windows installer path exits
/// the process directly, so the updater runs this first.
fn before_exit(app: &AppHandle) {
    app.cleanup_before_exit();
    crate::socket::cleanup(app);
    if let Some(recorder) = app.try_state::<crate::frame_ring::Recorder>() {
        recorder.stop();
    }
}

/// Asks the feed for a newer release than the running app.
async fn fetch(
    app: &AppHandle,
) -> Result<Option<(Update, UpdateInfo)>, tauri_plugin_updater::Error> {
    let handle = app.clone();
    let found = app
        .updater_builder()
        .on_before_exit(move || before_exit(&handle))
        .build()?
        .check()
        .await?;
    let current = current(app);
    Ok(found
        .map(|u| {
            let info = info_of(&u);
            (u, info)
        })
        .filter(|(_, info)| is_newer(&current, &info.version)))
}

/// Starts the quiet checks: the first 15 s after launch, then every 6 hours.
/// Does nothing when automatic checks are off at launch; switching them off
/// later skips the checks that follow.
pub fn spawn_background_checks(app: &AppHandle, enabled: bool) {
    if !enabled {
        return;
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let mut log = BackgroundLog::default();
        tokio::time::sleep(FIRST_CHECK).await;
        loop {
            // Switched off since launch: skip the checks until it is back on.
            if still_enabled(&app) {
                let result = fetch(&app)
                    .await
                    .map(|found| found.map(|(_, info)| info))
                    .map_err(|e| e.to_string());
                let outcome = background_outcome(&current(&app), result, &mut log);
                if let Some(line) = outcome.log {
                    eprintln!("shdc: {line}");
                }
                if outcome.forget {
                    with_state(&app, |s| s.available = None);
                }
                if let Some(phase) = outcome.emit {
                    emit(&app, &phase);
                }
            }
            tokio::time::sleep(CHECK_EVERY).await;
        }
    });
}

/// The current `auto_check_updates` setting (on when unknown).
fn still_enabled(app: &AppHandle) -> bool {
    app.try_state::<Mutex<crate::settings::AppSettings>>()
        .and_then(|s| s.lock().ok().map(|s| s.auto_check_updates))
        .unwrap_or(true)
}

/// The running app's version.
#[tauri::command]
pub fn current_version(app: AppHandle) -> String {
    current(&app)
}

/// The last release found, the version put off with Later and the install's
/// progress (no network).
#[tauri::command]
pub fn update_status(state: State<'_, UpdateState>) -> Snapshot {
    let mut snapshot = state.inner.lock().map(|s| s.clone()).unwrap_or_default();
    snapshot.installing = state.installing.load(Ordering::SeqCst);
    snapshot
}

/// Later: hides this version's notice in every window until the next launch.
#[tauri::command]
pub fn dismiss_update(app: AppHandle, state: State<'_, UpdateState>, version: String) {
    if let Ok(mut s) = state.inner.lock() {
        s.dismissed = Some(version.clone());
    }
    let _ = app.emit(DISMISSED_EVENT, Some(version));
}

/// Check now: emits `Available` or `UpToDate` and returns the release found;
/// a failure is emitted as `Failed` and returned as its text. Asking again
/// shows a version that was put off with Later.
#[tauri::command]
pub async fn check_update(
    app: AppHandle,
    state: State<'_, UpdateState>,
) -> Result<Option<UpdateInfo>, String> {
    match fetch(&app).await {
        Ok(found) => {
            let info = found.map(|(_, info)| info);
            if let Ok(mut s) = state.inner.lock() {
                s.available = info.clone();
                s.dismissed = None;
            }
            let _ = app.emit(DISMISSED_EVENT, None::<String>);
            emit(&app, &info.clone().map_or(Phase::UpToDate, Phase::from));
            Ok(info)
        }
        Err(e) => {
            eprintln!("shdc: update check failed: {e}");
            Err(fail(&app, classify(&e)))
        }
    }
}

/// Emits a failure and returns its text.
fn fail(app: &AppHandle, phase: Phase) -> String {
    emit(app, &phase);
    match phase {
        Phase::Failed { error } => error,
        other => format!("{other:?}"),
    }
}

/// Update now: checks again, downloads with progress, lets the plugin verify
/// the signature, installs, then relaunches as the new version. Only ever
/// called from the user's click. On Windows the installer takes over and
/// relaunches the app itself.
#[tauri::command]
pub async fn install_update(app: AppHandle, state: State<'_, UpdateState>) -> Result<(), String> {
    if state.installing.swap(true, Ordering::SeqCst) {
        return Err("An update is already being installed".into());
    }
    let result = install(&app).await;
    state.installing.store(false, Ordering::SeqCst);
    result.map_err(|phase| fail(&app, phase))
}

async fn install(app: &AppHandle) -> Result<(), Phase> {
    let (update, _) = match fetch(app).await {
        Ok(Some(found)) => found,
        Ok(None) => {
            with_state(app, |s| s.available = None);
            emit(app, &Phase::UpToDate);
            return Ok(());
        }
        Err(e) => {
            eprintln!("shdc: update check failed: {e}");
            return Err(classify(&e));
        }
    };
    let send = |p: Progress| next_step(p).emit.iter().for_each(|ph| emit(app, ph));
    // Bytes so far, the total and the progress gate, shared by the callbacks.
    let sofar = Mutex::new((0u64, None::<u64>, PercentGate::default()));
    // `download` calls the finish callback, then checks the signature.
    let bytes = update
        .download(
            |chunk, length| {
                let Ok(mut s) = sofar.lock() else { return };
                s.0 += chunk as u64;
                s.1 = length;
                let (downloaded, total) = (s.0, s.1);
                if s.2.pass(downloaded, total) {
                    drop(s);
                    send(Progress::Chunk { downloaded, total });
                }
            },
            || {
                let (downloaded, total) = sofar.lock().map(|s| (s.0, s.1)).unwrap_or_default();
                send(Progress::DownloadFinished { downloaded, total });
            },
        )
        .await
        .map_err(|e| {
            eprintln!("shdc: update not downloaded: {e}");
            classify(&e)
        })?;
    send(Progress::Verified);
    // `install` checks the signature once more before it touches anything.
    if let Err(e) = update.install(&bytes) {
        eprintln!("shdc: update not installed: {e}");
        return Err(classify(&e));
    }
    let step = next_step(Progress::Installed(Ok(())));
    step.emit.iter().for_each(|ph| emit(app, ph));
    if step.restart {
        app.restart();
    }
    Ok(())
}
