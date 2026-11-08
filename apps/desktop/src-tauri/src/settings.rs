//! App settings (theme, launch at login, Dock icon, update checks), stored as `settings.json`
//! in the app config directory.

use std::io;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};

use crate::broker::BrokerCmd;

const FILE_NAME: &str = "settings.json";

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    /// The default: the light SHDC palette.
    #[default]
    Paper,
    Ink,
    /// Follows the system's light/dark appearance.
    System,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct AppSettings {
    pub theme: Theme,
    pub launch_at_login: bool,
    pub hide_dock_icon: bool,
    /// Shows the Advanced screen (frame log, capabilities, raw command).
    #[serde(default)]
    pub show_advanced: bool,
    /// Checks GitHub for a newer release on launch and every 6 hours; read
    /// at launch. On unless switched off.
    #[serde(default = "yes")]
    pub auto_check_updates: bool,
}

fn yes() -> bool {
    true
}

impl Default for AppSettings {
    fn default() -> Self {
        AppSettings {
            theme: Theme::Paper,
            launch_at_login: false,
            hide_dock_icon: true,
            show_advanced: false,
            auto_check_updates: true,
        }
    }
}

/// Raw commands are allowed only while the Advanced panel is enabled.
pub fn raw_allowed(s: &AppSettings) -> Result<(), String> {
    if s.show_advanced {
        Ok(())
    } else {
        Err("enable the Advanced panel in Settings first".into())
    }
}

/// What to tell the broker when the settings change from `before` to
/// `after`: switching the Advanced panel off stops the probe and the frame
/// recording. Other saves send nothing (so a `SHDC_FRAME_LOG=1`
/// recording survives a theme change).
pub fn advanced_off_commands(before: &AppSettings, after: &AppSettings) -> Vec<BrokerCmd> {
    if before.show_advanced && !after.show_advanced {
        vec![BrokerCmd::ProbeStop, BrokerCmd::RecordFrames(false)]
    } else {
        Vec::new()
    }
}

fn path_in(dir: &Path) -> PathBuf {
    dir.join(FILE_NAME)
}

/// The settings in `dir`; defaults when the file is missing or unreadable.
pub fn load(dir: &Path) -> AppSettings {
    std::fs::read_to_string(path_in(dir))
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

/// Writes the settings to `dir` as pretty JSON, creating `dir` if needed.
/// Written to a `.tmp` file first and renamed, so a crash never leaves a
/// half-written file.
pub fn save(dir: &Path, s: &AppSettings) -> io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let text = serde_json::to_string_pretty(s).map_err(io::Error::other)?;
    let path = path_in(dir);
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, text)?;
    std::fs::rename(&tmp, &path)
}

/// The app's settings directory.
pub fn dir(app: &AppHandle) -> tauri::Result<PathBuf> {
    app.path().app_config_dir()
}

#[tauri::command]
pub fn get_settings(state: State<'_, Mutex<AppSettings>>) -> AppSettings {
    state.lock().map(|s| s.clone()).unwrap_or_default()
}

/// Saves the settings, applies launch-at-login and the Dock policy, stops
/// the probe and the recording when the Advanced panel is switched off, and
/// tells every window to re-read them.
#[tauri::command]
pub fn set_settings(
    app: AppHandle,
    state: State<'_, Mutex<AppSettings>>,
    s: AppSettings,
) -> Result<(), String> {
    let dir = dir(&app).map_err(|e| e.to_string())?;
    save(&dir, &s).map_err(|e| format!("could not save settings: {e}"))?;
    let mut before = None;
    if let Ok(mut current) = state.lock() {
        before = Some(std::mem::replace(&mut *current, s.clone()));
    }
    let stop = before.map_or_else(Vec::new, |b| advanced_off_commands(&b, &s));
    if let Some(broker) = app.try_state::<crate::commands::Broker>() {
        for cmd in stop {
            if let Err(e) = broker.send(cmd) {
                eprintln!("shdc: {e}");
            }
        }
    }
    apply_launch_at_login(&app, s.launch_at_login);
    crate::dock::apply_for(
        &app,
        app.get_webview_window("main").is_some(),
        s.hide_dock_icon,
    );
    let _ = app.emit("settings", &s);
    Ok(())
}

/// Registers or removes the login item. Failure is logged, never fatal.
fn apply_launch_at_login(app: &AppHandle, on: bool) {
    use tauri_plugin_autostart::ManagerExt;
    let autolaunch = app.autolaunch();
    if autolaunch.is_enabled().is_ok_and(|enabled| enabled == on) {
        return;
    }
    let result = if on {
        autolaunch.enable()
    } else {
        autolaunch.disable()
    };
    if let Err(e) = result {
        eprintln!("shdc: launch at login not changed: {e}");
    }
}

/// Makes `launch_at_login` match the login item the system actually has (it
/// can be removed in System Settings) and saves the reconciled value to `dir`.
pub fn reconcile_launch_at_login(
    app: &AppHandle,
    dir: Option<&Path>,
    mut s: AppSettings,
) -> AppSettings {
    use tauri_plugin_autostart::ManagerExt;
    let enabled = match app.autolaunch().is_enabled() {
        Ok(enabled) => enabled,
        Err(e) => {
            eprintln!("shdc: launch at login state unknown ({e}); keeping the setting");
            return s;
        }
    };
    if enabled == s.launch_at_login {
        return s;
    }
    eprintln!(
        "shdc: launch at login was {} in settings but the login item says {enabled}; using the login item",
        s.launch_at_login
    );
    s.launch_at_login = enabled;
    if let Some(dir) = dir {
        if let Err(e) = save(dir, &s) {
            eprintln!("shdc: could not save settings: {e}");
        }
    }
    s
}
