//! In-app updates from GitHub Releases, on top of `tauri-plugin-updater`.
//!
//! The plugin reads `latest.json` from the latest release (the endpoint and
//! the public key are in `tauri.conf.json`) and refuses any download whose
//! minisign signature does not match the key. This module adds the app's
//! policy: a quiet background check (15 s after launch, then every 6 hours,
//! only when `auto_check_updates` is on), a check the user asks for, and an
//! install that runs only from the user's Update now click. Progress goes to
//! every window as `"update"` events carrying a [`Phase`].

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
    Installing,
    Restarting,
    Failed {
        error: String,
    },
    UpToDate,
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

/// What to tell the user about a failed check or install, from the error's
/// text: a signature problem means nothing was installed, a network problem
/// means GitHub could not be reached; anything else keeps its own words.
pub fn classify_error(e: &str) -> Phase {
    let lower = e.to_lowercase();
    let error = if ["signature", "minisign", "verif", "public key", "base64"]
        .iter()
        .any(|w| lower.contains(w))
    {
        NOT_VERIFIED.to_string()
    } else if [
        "error sending request",
        "request failed",
        "dns",
        "connect",
        "timed out",
        "network",
        "release json",
        "status:",
    ]
    .iter()
    .any(|w| lower.contains(w))
    {
        UNREACHABLE.to_string()
    } else {
        format!("Update failed: {e}")
    };
    Phase::Failed { error }
}

/// [`classify_error`] on the plugin's error, by kind where the kind says it.
pub fn classify(e: &tauri_plugin_updater::Error) -> Phase {
    use tauri_plugin_updater::Error as E;
    match e {
        E::Minisign(_) | E::Base64(_) | E::SignatureUtf8(_) => Phase::Failed {
            error: NOT_VERIFIED.into(),
        },
        E::Reqwest(_) | E::Network(_) | E::ReleaseNotFound => Phase::Failed {
            error: UNREACHABLE.into(),
        },
        other => classify_error(&other.to_string()),
    }
}

/// Remembers whether the last background check failed, so an offline
/// machine logs one line per outage instead of one per check.
#[derive(Debug, Default)]
pub struct BackgroundLog {
    failing: bool,
}

/// What a background check does with its result: emit `Available` for a
/// newer release, and otherwise nothing. A failure returns a log line only
/// when it starts a run of failures.
pub fn background_outcome(
    current: &str,
    result: Result<Option<UpdateInfo>, String>,
    log: &mut BackgroundLog,
) -> (Option<Phase>, Option<String>) {
    match result {
        Err(e) => {
            let line = (!log.failing).then(|| format!("update check failed: {e}"));
            log.failing = true;
            (None, line)
        }
        Ok(found) => {
            log.failing = false;
            let emit = found
                .filter(|i| is_newer(current, &i.version))
                .map(Phase::from);
            (emit, None)
        }
    }
}

/// One thing that happened during an install.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Progress {
    /// Bytes so far, out of `total` when the server said.
    Chunk { downloaded: u64, total: Option<u64> },
    /// The download is complete and verified; installing starts now.
    DownloadFinished { downloaded: u64, total: Option<u64> },
    /// The installer's result.
    Installed(Result<(), String>),
}

/// What to emit for a [`Progress`], and whether to relaunch now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    pub emit: Vec<Phase>,
    pub restart: bool,
}

/// The install sequence: the bar reaches 100 % before `Installing`, and the
/// relaunch happens only after the installer returned Ok.
pub fn next_step(p: Progress) -> Step {
    let clamp =
        |downloaded: u64, total: Option<u64>| total.map_or(downloaded, |t| downloaded.min(t));
    match p {
        Progress::Chunk { downloaded, total } => Step {
            emit: vec![Phase::Downloading {
                downloaded: clamp(downloaded, total),
                total,
            }],
            restart: false,
        },
        Progress::DownloadFinished { downloaded, total } => Step {
            emit: vec![
                Phase::Downloading {
                    downloaded: total.unwrap_or(downloaded),
                    total,
                },
                Phase::Installing,
            ],
            restart: false,
        },
        Progress::Installed(Ok(())) => Step {
            emit: vec![Phase::Restarting],
            restart: true,
        },
        Progress::Installed(Err(e)) => Step {
            emit: vec![classify_error(&e)],
            restart: false,
        },
    }
}

/// The app's memory of the update: the last release found and the version
/// put off with Later. Kept for windows opened later; lost on relaunch.
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

fn emit(app: &AppHandle, phase: &Phase) {
    let _ = app.emit(EVENT, phase);
}

fn remember(app: &AppHandle, available: Option<UpdateInfo>) {
    if let Some(state) = app.try_state::<UpdateState>() {
        if let Ok(mut s) = state.inner.lock() {
            s.available = available;
        }
    }
}

/// Asks the feed for a newer release than the running app.
async fn fetch(
    app: &AppHandle,
) -> Result<Option<(Update, UpdateInfo)>, tauri_plugin_updater::Error> {
    let found = app.updater()?.check().await?;
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
            if !still_enabled(&app) {
                tokio::time::sleep(CHECK_EVERY).await;
                continue;
            }
            let result = fetch(&app)
                .await
                .map(|found| found.map(|(_, info)| info))
                .map_err(|e| e.to_string());
            let (phase, line) = background_outcome(&current(&app), result, &mut log);
            if let Some(line) = line {
                eprintln!("hairline: {line}");
            }
            if let Some(phase) = phase {
                if let Phase::Available {
                    version,
                    notes,
                    date,
                } = phase.clone()
                {
                    remember(
                        &app,
                        Some(UpdateInfo {
                            version,
                            notes,
                            date,
                        }),
                    );
                }
                emit(&app, &phase);
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

/// The last release found and the version put off with Later.
#[tauri::command]
pub fn update_status(state: State<'_, UpdateState>) -> Snapshot {
    state.inner.lock().map(|s| s.clone()).unwrap_or_default()
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
            eprintln!("hairline: update check failed: {e}");
            let phase = classify(&e);
            emit(&app, &phase);
            Err(match phase {
                Phase::Failed { error } => error,
                _ => e.to_string(),
            })
        }
    }
}

/// Update now: checks again, downloads with progress, lets the plugin verify
/// the signature and install, then relaunches as the new version. Only ever
/// called from the user's click. On Windows the installer takes over and
/// relaunches the app itself.
#[tauri::command]
pub async fn install_update(app: AppHandle, state: State<'_, UpdateState>) -> Result<(), String> {
    if state.installing.swap(true, Ordering::SeqCst) {
        return Err("An update is already being installed".into());
    }
    let result = install(&app).await;
    state.installing.store(false, Ordering::SeqCst);
    if let Err(phase) = &result {
        emit(&app, phase);
    }
    result.map_err(|phase| match phase {
        Phase::Failed { error } => error,
        other => format!("{other:?}"),
    })
}

async fn install(app: &AppHandle) -> Result<(), Phase> {
    let (update, _) = match fetch(app).await {
        Ok(Some(found)) => found,
        Ok(None) => {
            remember(app, None);
            emit(app, &Phase::UpToDate);
            return Ok(());
        }
        Err(e) => {
            eprintln!("hairline: update check failed: {e}");
            return Err(classify(&e));
        }
    };
    // Bytes so far and the total, shared by the two callbacks.
    let sofar = Mutex::new((0u64, None::<u64>));
    let progress = |p: Progress| next_step(p).emit.iter().for_each(|ph| emit(app, ph));
    let installed = update
        .download_and_install(
            |chunk, length| {
                let (downloaded, total) = match sofar.lock() {
                    Ok(mut s) => {
                        s.0 += chunk as u64;
                        s.1 = length;
                        *s
                    }
                    Err(_) => return,
                };
                progress(Progress::Chunk { downloaded, total });
            },
            || {
                let (downloaded, total) = sofar.lock().map(|s| *s).unwrap_or_default();
                progress(Progress::DownloadFinished { downloaded, total });
            },
        )
        .await;
    if let Err(e) = &installed {
        eprintln!("hairline: update not installed: {e}");
        return Err(classify(e));
    }
    let step = next_step(Progress::Installed(Ok(())));
    step.emit.iter().for_each(|ph| emit(app, ph));
    if step.restart {
        app.restart();
    }
    Ok(())
}
