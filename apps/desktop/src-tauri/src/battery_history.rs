//! Battery history: `battery.jsonl` beside `settings.json`, one sample per
//! line (`{"t":<unix secs>,"level":<0..=100>,"charging":<bool>}`), and the
//! "time left" estimate drawn from it.
//!
//! A sample is written when the level or the charging flag changes, plus one
//! on each connect. The file keeps the newest `CAP` samples.
//!
//! Time left is a least-squares line through the current discharge segment:
//! the trailing samples that are not charging with no gap over 30 min between
//! neighbours (nor between the last one and now). It is shown only when that
//! segment spans at least 30 min and drops at least 2 %, and the line falls;
//! otherwise it is blank, never invented.

use std::fs::OpenOptions;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use mdr_proto::Charging;
use mdr_session::{DeviceState, Status};
use serde::{Deserialize, Serialize};

const FILE_NAME: &str = "battery.jsonl";
/// The most samples the file keeps; older ones are rewritten away.
pub const CAP: usize = 5000;
/// A longer silence between samples ends a discharge segment.
const MAX_GAP_S: u64 = 30 * 60;
/// The shortest segment an estimate is drawn from.
const MIN_SPAN_S: u64 = 30 * 60;
/// The smallest drop, in percent, an estimate is drawn from.
const MIN_DROP: u8 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sample {
    /// Unix seconds.
    pub t: u64,
    pub level: u8,
    pub charging: bool,
}

/// The `"battery"` event payload and the `battery_estimate` answer.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct BatteryEstimate {
    /// Rounded to the nearest 10 min; `None` when there is not enough data.
    pub minutes_left: Option<u32>,
    /// Samples in the file.
    pub samples: usize,
    /// The oldest sample's time (unix seconds).
    pub since: Option<u64>,
}

fn path(dir: &Path) -> PathBuf {
    dir.join(FILE_NAME)
}

/// Every sample in `dir`, oldest first; unreadable lines are skipped and a
/// missing file is empty.
pub fn load(dir: &Path) -> Vec<Sample> {
    std::fs::read_to_string(path(dir))
        .map(|text| {
            text.lines()
                .filter_map(|line| serde_json::from_str(line).ok())
                .collect()
        })
        .unwrap_or_default()
}

/// Writes `sample` unless the level and charging flag match the last one.
/// Returns whether it was written.
pub fn append(dir: &Path, sample: Sample) -> io::Result<bool> {
    let samples = load(dir);
    let same = samples
        .last()
        .is_some_and(|l| (l.level, l.charging) == (sample.level, sample.charging));
    if same {
        return Ok(false);
    }
    write(dir, samples, sample)?;
    Ok(true)
}

/// Writes `sample` whatever the last one was (the sample taken on connect).
pub fn push(dir: &Path, sample: Sample) -> io::Result<()> {
    write(dir, load(dir), sample)
}

/// Appends a line, or rewrites the file through a `.tmp` without the oldest
/// samples once it would exceed `CAP`.
fn write(dir: &Path, mut samples: Vec<Sample>, sample: Sample) -> io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let line = |s: &Sample| serde_json::to_string(s).map_err(io::Error::other);
    if samples.len() < CAP {
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path(dir))?;
        return writeln!(file, "{}", line(&sample)?);
    }
    samples.push(sample);
    let keep = &samples[samples.len() - CAP..];
    let mut text = String::new();
    for s in keep {
        text.push_str(&line(s)?);
        text.push('\n');
    }
    let tmp = dir.join(format!("{FILE_NAME}.tmp"));
    std::fs::write(&tmp, text)?;
    std::fs::rename(&tmp, path(dir))
}

/// Deletes the history; nothing to delete is fine.
pub fn clear(dir: &Path) -> io::Result<()> {
    match std::fs::remove_file(path(dir)) {
        Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e),
        _ => Ok(()),
    }
}

/// The estimate for `samples` (oldest first) at `now` (unix seconds).
pub fn estimate(samples: &[Sample], now: u64) -> BatteryEstimate {
    BatteryEstimate {
        minutes_left: minutes_left(samples, now),
        samples: samples.len(),
        since: samples.first().map(|s| s.t),
    }
}

fn minutes_left(samples: &[Sample], now: u64) -> Option<u32> {
    let last = samples.last()?;
    if last.charging || now.saturating_sub(last.t) > MAX_GAP_S {
        return None;
    }
    // The trailing discharge segment.
    let mut start = samples.len() - 1;
    while start > 0 {
        let prev = &samples[start - 1];
        if prev.charging || samples[start].t.saturating_sub(prev.t) > MAX_GAP_S {
            break;
        }
        start -= 1;
    }
    let segment = &samples[start..];
    let first = &segment[0];
    if last.t.saturating_sub(first.t) < MIN_SPAN_S
        || first.level.saturating_sub(last.level) < MIN_DROP
    {
        return None;
    }
    // Least squares over (seconds since the first sample, level).
    let n = segment.len() as f64;
    let xs = segment.iter().map(|s| (s.t - first.t) as f64);
    let mean_x = xs.clone().sum::<f64>() / n;
    let mean_y = segment.iter().map(|s| f64::from(s.level)).sum::<f64>() / n;
    let (mut sxy, mut sxx) = (0.0, 0.0);
    for (x, s) in xs.zip(segment) {
        sxy += (x - mean_x) * (f64::from(s.level) - mean_y);
        sxx += (x - mean_x) * (x - mean_x);
    }
    let slope = sxy / sxx; // percent per second
    if !slope.is_finite() || slope >= 0.0 {
        return None;
    }
    let minutes = f64::from(last.level) / -slope / 60.0;
    let rounded = ((minutes / 10.0).round() * 10.0) as u32;
    (rounded > 0).then_some(rounded)
}

/// Unix seconds now.
pub fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// The broker's battery sampler: writes a sample when the confirmed battery
/// value changes (and the first one after each connect) and emits the new
/// estimate as the `"battery"` event.
pub struct BatteryHost<'a> {
    dir: PathBuf,
    /// Unix seconds.
    wall: Box<dyn Fn() -> u64 + 'a>,
    emit: Box<dyn Fn(&BatteryEstimate) + 'a>,
    /// The last value seen on this link; `None` until the first one.
    last: Option<(u8, bool)>,
}

impl<'a> BatteryHost<'a> {
    pub fn new(
        dir: PathBuf,
        wall: Box<dyn Fn() -> u64 + 'a>,
        emit: Box<dyn Fn(&BatteryEstimate) + 'a>,
    ) -> BatteryHost<'a> {
        BatteryHost {
            dir,
            wall,
            emit,
            last: None,
        }
    }

    /// A new connection: its first battery value is written whatever it is.
    pub fn new_link(&mut self) {
        self.last = None;
    }

    /// Looks at the state after a broker pass; writes and emits on a change.
    pub fn observe(&mut self, state: &DeviceState) {
        if !state.connected || state.battery.status != Status::Confirmed {
            return;
        }
        let Some((level, charging)) = state.battery.value else {
            return;
        };
        // Anything but "not charging" (charging, charged, unknown) is not a
        // discharge, so it never feeds an estimate.
        let value = (level, charging != Charging::NotCharging);
        if self.last == Some(value) {
            return;
        }
        let first = self.last.is_none();
        self.last = Some(value);
        let sample = Sample {
            t: (self.wall)(),
            level: value.0,
            charging: value.1,
        };
        let written = if first {
            push(&self.dir, sample).map(|()| true)
        } else {
            append(&self.dir, sample)
        };
        match written {
            Ok(true) => self.emit_current(),
            Ok(false) => {}
            Err(e) => eprintln!("hairline: battery sample not saved: {e}"),
        }
    }

    /// Emits the estimate from the file as it stands.
    pub fn emit_current(&self) {
        (self.emit)(&estimate(&load(&self.dir), (self.wall)()));
    }
}
