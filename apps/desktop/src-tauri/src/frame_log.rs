//! Frame recording: every frame and session log line, appended to
//! `<capture dir>/<YYYY-MM-DD-HHMMSS>.log`, where the capture dir is
//! `captures` in the app's log directory (`capture_dir`). Started from the
//! Advanced panel (`record_frames`), or at launch with `SHDC_FRAME_LOG=1`.
//!
//! Lines are `t_ms dir data`: `>` frames we sent, `<` bytes the device sent
//! (as received, so a chunk can hold part of a frame or several), both as
//! uppercase hex, and `#` session log messages.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use tauri::{AppHandle, Manager};

pub struct FrameLog {
    file: File,
    path: PathBuf,
}

/// Log file name for a moment in UTC, as `YYYY-MM-DD-HHMMSS.log`.
pub fn file_name(unix_secs: u64) -> String {
    let days = (unix_secs / 86_400) as i64;
    let secs = unix_secs % 86_400;
    let (y, m, d) = civil_from_days(days);
    format!(
        "{y:04}-{m:02}-{d:02}-{:02}{:02}{:02}.log",
        secs / 3600,
        secs / 60 % 60,
        secs % 60
    )
}

/// Proleptic Gregorian date of a day count since 1970-01-01 (Howard Hinnant's algorithm).
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = yoe + era * 400 + i64::from(m <= 2);
    (y, m, d)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02X}")).collect()
}

impl FrameLog {
    /// Creates `dir` if missing and opens a log named after `unix_secs`.
    pub fn create_in(dir: &Path, unix_secs: u64) -> io::Result<FrameLog> {
        fs::create_dir_all(dir)?;
        let path = dir.join(file_name(unix_secs));
        let file = OpenOptions::new().create(true).append(true).open(&path)?;
        Ok(FrameLog { file, path })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// A frame we sent.
    pub fn sent(&mut self, t_ms: u64, bytes: &[u8]) {
        self.line(t_ms, ">", &hex(bytes));
    }

    /// Bytes the device sent.
    pub fn received(&mut self, t_ms: u64, bytes: &[u8]) {
        self.line(t_ms, "<", &hex(bytes));
    }

    /// A session log message.
    pub fn note(&mut self, t_ms: u64, msg: &str) {
        self.line(t_ms, "#", msg);
    }

    fn line(&mut self, t_ms: u64, dir: &str, data: &str) {
        // A failed write must not take the link down; the log is best effort.
        let _ = self
            .file
            .write_all(format!("{t_ms} {dir} {data}\n").as_bytes());
    }
}

/// The capture directory's name inside the app's log directory.
pub const CAPTURES: &str = "captures";

/// Where frame logs and probe results go: `captures` in the log directory,
/// else in the config directory, else in the system temp directory.
pub fn capture_dir(log_dir: Option<PathBuf>, config_dir: Option<PathBuf>) -> PathBuf {
    log_dir
        .or(config_dir)
        .unwrap_or_else(|| std::env::temp_dir().join("shdc"))
        .join(CAPTURES)
}

/// `capture_dir` for the running app (on macOS
/// `~/Library/Logs/com.drake0306.hairline/captures`).
pub fn app_capture_dir(app: &AppHandle) -> PathBuf {
    let path = app.path();
    capture_dir(path.app_log_dir().ok(), path.app_config_dir().ok())
}

/// The capture directory, held in Tauri state for `record_frames`; the
/// broker's frame tap and probe use the same one.
pub struct CaptureDir(pub PathBuf);
