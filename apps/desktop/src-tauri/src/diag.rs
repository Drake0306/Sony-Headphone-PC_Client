//! A small diagnostics log for what a user cannot see: window creation, page
//! loads, and errors the web page reports. One line per event in `shdc.log`
//! in the app's log directory (macOS `~/Library/Logs/com.drake0306.hairline/`,
//! Windows `%LOCALAPPDATA%\\com.drake0306.hairline\\logs\\`), also on stderr.
//! Started because the Windows main window came up white with nothing to
//! read. The file is cut back to nothing once it passes half a megabyte.

use std::io::Write;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use tauri::{AppHandle, Manager};

pub const FILE: &str = "shdc.log";
const MAX_BYTES: u64 = 512 * 1024;

/// `shdc.log` in the app's log directory, when the platform names one.
pub fn path(app: &AppHandle) -> Option<PathBuf> {
    app.path().app_log_dir().ok().map(|d| d.join(FILE))
}

/// One log line: the Unix time in seconds, then the message.
pub fn line(unix_secs: u64, message: &str) -> String {
    format!("{unix_secs} {}\n", message.trim())
}

/// Appends a line to the log and echoes it to stderr. Never fails the caller.
pub fn log(app: &AppHandle, message: &str) {
    eprintln!("shdc: {message}");
    let Some(path) = path(app) else {
        return;
    };
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let too_big = std::fs::metadata(&path).is_ok_and(|m| m.len() > MAX_BYTES);
    let file = std::fs::OpenOptions::new()
        .create(true)
        .append(!too_big)
        .write(true)
        .truncate(too_big)
        .open(&path);
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    if let Ok(mut f) = file {
        let _ = f.write_all(line(secs, message).as_bytes());
    }
}
