//! The local socket the `hairlinectl` command-line tool talks to.
//!
//! One JSON object per line in, one JSON object per line out:
//! `state`, `set` (the controls in `commands::SOCKET_CONTROLS`, validated by
//! `commands::control_command`), `scene`, `scenes`, `battery`. Nothing else:
//! no raw commands, no power off, no probe, no reset. A line that does not
//! parse gets an error line and the connection stays open.
//!
//! The socket is `hairline.sock` in the settings directory, mode 0600, and
//! is removed when the app exits. Unix only; elsewhere the server is not
//! started.

use std::sync::Mutex;

use mdr_session::DeviceState;
use serde_json::{json, Map, Value};
use tauri::{AppHandle, Manager};

use crate::battery_history::{self, BatteryEstimate};
use crate::broker::BrokerCmd;
use crate::commands::{control_command, Broker};
use crate::scenes::model::Scene;
use crate::settings;

/// The socket file's name inside the settings directory.
pub const SOCKET_NAME: &str = "hairline.sock";
/// Longest request line accepted; a longer one closes the connection.
pub const MAX_LINE: usize = 64 * 1024;

/// The active scene id from the latest `"scenes"` event, held in Tauri
/// state for the socket's `scenes` answer.
#[derive(Default)]
pub struct ActiveScene(pub Mutex<Option<String>>);

/// What the protocol reads and where it sends commands; the app wraps its
/// `AppHandle`, tests use a fake.
pub trait SocketContext {
    fn state(&self) -> DeviceState;
    /// The scene list and the active scene's id.
    fn scenes(&self) -> (Vec<Scene>, Option<String>);
    fn battery(&self) -> BatteryEstimate;
    fn send(&self, cmd: BrokerCmd) -> Result<(), String>;
}

fn error(text: &str) -> Value {
    json!({"ok": false, "error": text})
}

/// Answers one request line (without its newline) with one JSON line
/// (without a newline).
pub fn handle_line(line: &str, ctx: &dyn SocketContext) -> String {
    let answer = match serde_json::from_str::<Value>(line) {
        Ok(Value::Object(req)) => handle(&req, ctx),
        _ => error("bad json"),
    };
    answer.to_string()
}

fn handle(req: &Map<String, Value>, ctx: &dyn SocketContext) -> Value {
    let done = |result: Result<(), String>| match result {
        Ok(()) => json!({"ok": true}),
        Err(e) => error(&e),
    };
    match req.get("cmd").and_then(Value::as_str) {
        Some("state") => json!({"ok": true, "state": ctx.state()}),
        Some("set") => done(set(req, ctx)),
        Some("scene") => done(scene(req, ctx)),
        Some("scenes") => {
            let (scenes, active) = ctx.scenes();
            json!({"ok": true, "scenes": scenes, "active": active})
        }
        Some("battery") => json!({"ok": true, "battery": ctx.battery()}),
        _ => error("unknown cmd"),
    }
}

fn set(req: &Map<String, Value>, ctx: &dyn SocketContext) -> Result<(), String> {
    let control = req
        .get("control")
        .and_then(Value::as_str)
        .ok_or("set needs a control")?;
    let mut value = req.get("value").cloned().unwrap_or(Value::Null);
    // `nc` may carry `level` and `voice` beside the mode.
    if control == "nc" {
        if let Value::String(mode) = &value {
            let mut o = Map::new();
            o.insert("mode".into(), Value::String(mode.clone()));
            for key in ["level", "voice"] {
                if let Some(v) = req.get(key) {
                    o.insert(key.into(), v.clone());
                }
            }
            value = Value::Object(o);
        }
    }
    let state = ctx.state();
    let cmd = control_command(control, &value, &state)?;
    if !state.connected {
        return Err("not connected".into());
    }
    ctx.send(cmd)
}

fn scene(req: &Map<String, Value>, ctx: &dyn SocketContext) -> Result<(), String> {
    let (scenes, _) = ctx.scenes();
    let found = if let Some(id) = req.get("id").and_then(Value::as_str) {
        scenes.iter().find(|s| s.id == id)
    } else if let Some(name) = req.get("name").and_then(Value::as_str) {
        let name = name.trim();
        scenes
            .iter()
            .find(|s| s.name.trim().to_lowercase() == name.to_lowercase())
    } else {
        return Err("scene needs a name or an id".into());
    };
    let scene = found.ok_or_else(|| {
        let what = req
            .get("name")
            .or_else(|| req.get("id"))
            .map(Value::to_string)
            .unwrap_or_default();
        format!("no scene {what}")
    })?;
    ctx.send(BrokerCmd::ApplyScene(scene.id.clone()))
}

/// The app's context: managed state and the broker channel. Only the Unix
/// server constructs it.
#[cfg_attr(not(unix), allow(dead_code))]
struct AppContext(AppHandle);

impl SocketContext for AppContext {
    fn state(&self) -> DeviceState {
        self.0
            .state::<Mutex<DeviceState>>()
            .lock()
            .map(|s| s.clone())
            .unwrap_or_default()
    }

    fn scenes(&self) -> (Vec<Scene>, Option<String>) {
        let scenes = self
            .0
            .state::<Mutex<Vec<Scene>>>()
            .lock()
            .map(|s| s.clone())
            .unwrap_or_default();
        let active = self
            .0
            .state::<ActiveScene>()
            .0
            .lock()
            .map(|a| a.clone())
            .unwrap_or_default();
        (scenes, active)
    }

    fn battery(&self) -> BatteryEstimate {
        settings::dir(&self.0)
            .map(|dir| {
                battery_history::estimate(&battery_history::load(&dir), battery_history::unix_now())
            })
            .unwrap_or_default()
    }

    fn send(&self, cmd: BrokerCmd) -> Result<(), String> {
        self.0.state::<Broker>().send(cmd)
    }
}

#[cfg(unix)]
pub use unix::{bind, serve};

#[cfg(unix)]
mod unix {
    use std::io::{self, BufRead, BufReader, Read, Write};
    use std::os::unix::fs::{FileTypeExt, PermissionsExt};
    use std::os::unix::net::{UnixListener, UnixStream};
    use std::path::Path;
    use std::sync::Arc;
    use std::time::Duration;

    use super::{error, handle_line, SocketContext, MAX_LINE};

    /// An idle client is dropped after this long, freeing its thread.
    pub const IDLE_TIMEOUT: Duration = Duration::from_secs(5 * 60);
    /// Wait after a failed accept before trying again.
    const ACCEPT_BACKOFF: Duration = Duration::from_millis(50);

    /// Listens on `path`, made readable and writable by this user only. A
    /// socket another Hairline still answers on is left alone (an error); a
    /// dead socket or stale file there is removed first.
    pub fn bind(path: &Path) -> io::Result<UnixListener> {
        if std::fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_socket())
            && UnixStream::connect(path).is_ok()
        {
            return Err(io::Error::new(
                io::ErrorKind::AddrInUse,
                format!("another Hairline is already serving {}", path.display()),
            ));
        }
        match std::fs::remove_file(path) {
            Err(e) if e.kind() != io::ErrorKind::NotFound => return Err(e),
            _ => {}
        }
        let listener = UnixListener::bind(path)?;
        if let Err(e) = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)) {
            let _ = std::fs::remove_file(path);
            return Err(e);
        }
        Ok(listener)
    }

    /// Accepts connections forever, one thread each.
    pub fn serve(listener: UnixListener, ctx: Arc<dyn SocketContext + Send + Sync>) {
        for stream in listener.incoming() {
            let Ok(stream) = stream else {
                // Out of descriptors, say: back off instead of spinning.
                std::thread::sleep(ACCEPT_BACKOFF);
                continue;
            };
            let ctx = ctx.clone();
            let _ = std::thread::Builder::new()
                .name("hairline-socket".into())
                .spawn(move || {
                    let _ = connection(stream, &*ctx);
                });
        }
    }

    fn connection(stream: UnixStream, ctx: &dyn SocketContext) -> io::Result<()> {
        stream.set_read_timeout(Some(IDLE_TIMEOUT))?;
        let mut writer = stream.try_clone()?;
        let mut reader = BufReader::new(stream);
        let mut buf = Vec::new();
        loop {
            buf.clear();
            let n = (&mut reader)
                .take(MAX_LINE as u64 + 1)
                .read_until(b'\n', &mut buf)?;
            if n == 0 {
                return Ok(());
            }
            if buf.last() != Some(&b'\n') && buf.len() > MAX_LINE {
                writeln!(writer, "{}", error("line too long"))?;
                return Ok(());
            }
            let answer = match std::str::from_utf8(&buf) {
                Ok(line) => {
                    let line = line.trim();
                    if line.is_empty() {
                        continue;
                    }
                    handle_line(line, ctx)
                }
                Err(_) => error("bad json").to_string(),
            };
            writeln!(writer, "{answer}")?;
        }
    }
}

/// Where the socket lives for this app.
pub fn path(app: &AppHandle) -> Option<std::path::PathBuf> {
    settings::dir(app).ok().map(|dir| dir.join(SOCKET_NAME))
}

/// Starts the server; call once everything it reads is managed.
#[cfg(unix)]
pub fn spawn(app: AppHandle) {
    let Some(path) = path(&app) else {
        eprintln!("hairline: no settings directory; the socket is off");
        return;
    };
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let listener = match bind(&path) {
        Ok(l) => l,
        Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => {
            eprintln!("hairline: {e}; this instance's socket is off");
            return;
        }
        Err(e) => {
            eprintln!("hairline: socket {} not started: {e}", path.display());
            return;
        }
    };
    OWNS_SOCKET.store(true, std::sync::atomic::Ordering::SeqCst);
    let ctx: std::sync::Arc<dyn SocketContext + Send + Sync> = std::sync::Arc::new(AppContext(app));
    let _ = std::thread::Builder::new()
        .name("hairline-socket-accept".into())
        .spawn(move || serve(listener, ctx));
}

/// The socket server is Unix only for now.
#[cfg(not(unix))]
pub fn spawn(_: AppHandle) {
    eprintln!("hairline: the socket is not supported on this system yet");
}

/// Set once this instance has bound the socket; only the owner removes it.
static OWNS_SOCKET: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Removes the socket file on exit, if this instance serves it.
pub fn cleanup(app: &AppHandle) {
    if OWNS_SOCKET.load(std::sync::atomic::Ordering::SeqCst) {
        if let Some(path) = path(app) {
            let _ = std::fs::remove_file(path);
        }
    }
}
