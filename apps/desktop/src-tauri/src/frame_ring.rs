//! The last frames on the wire, for the Advanced screen, and the switchable
//! frame recording (`FrameLog`).

use std::collections::VecDeque;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use mdr_proto::Table;
use serde::Serialize;

use crate::frame_log::FrameLog;

/// How many frames the ring keeps.
pub const RING_CAPACITY: usize = 500;
const FRAME_START: u8 = 0x3E;
const FRAME_END: u8 = 0x3C;
/// Received bytes without a frame end are flushed as one entry past this size.
const MAX_PARTIAL: usize = 4096;
/// Shortest gap, in ms, between two `"frames"` emits while frames keep coming.
pub const FRAMES_EVERY_MS: u64 = 200;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum FrameDir {
    Sent,
    Received,
}

/// One frame as it went over the wire (start and end markers, escapes and
/// checksum included). `table` is `None` for ACKs and for bytes that are not
/// a whole frame.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FrameEntry {
    pub t_ms: u64,
    pub dir: FrameDir,
    pub table: Option<Table>,
    pub bytes: Vec<u8>,
}

/// The table a wire frame belongs to, from its type byte.
fn table_of(frame: &[u8]) -> Option<Table> {
    match frame {
        [FRAME_START, 0x0C, ..] => Some(Table::T1),
        [FRAME_START, 0x0E, ..] => Some(Table::T2),
        _ => None,
    }
}

/// A bounded ring of frames, oldest first.
pub struct FrameRing {
    entries: VecDeque<FrameEntry>,
    capacity: usize,
    /// Received bytes of a frame whose end has not arrived yet.
    partial: Vec<u8>,
}

impl FrameRing {
    pub fn new(capacity: usize) -> FrameRing {
        FrameRing {
            entries: VecDeque::with_capacity(capacity),
            capacity,
            partial: Vec::new(),
        }
    }

    pub fn push(&mut self, entry: FrameEntry) {
        if self.entries.len() == self.capacity {
            self.entries.pop_front();
        }
        self.entries.push_back(entry);
    }

    /// A frame we sent (always whole).
    pub fn push_sent(&mut self, t_ms: u64, frame: &[u8]) {
        self.push(FrameEntry {
            t_ms,
            dir: FrameDir::Sent,
            table: table_of(frame),
            bytes: frame.to_vec(),
        });
    }

    /// A chunk the device sent: split into frames at the end marker (escaped
    /// inside a frame, so unambiguous); an unfinished frame waits for the next chunk.
    pub fn push_received(&mut self, t_ms: u64, chunk: &[u8]) {
        self.partial.extend_from_slice(chunk);
        while let Some(end) = self.partial.iter().position(|&b| b == FRAME_END) {
            let frame: Vec<u8> = self.partial.drain(..=end).collect();
            self.push_received_frame(t_ms, frame);
        }
        if self.partial.len() > MAX_PARTIAL {
            let bytes = std::mem::take(&mut self.partial);
            self.push_received_frame(t_ms, bytes);
        }
    }

    fn push_received_frame(&mut self, t_ms: u64, bytes: Vec<u8>) {
        self.push(FrameEntry {
            t_ms,
            dir: FrameDir::Received,
            table: table_of(&bytes),
            bytes,
        });
    }

    /// Drops an unfinished received frame (the link it came from is gone).
    pub fn reset_partial(&mut self) {
        self.partial.clear();
    }

    pub fn snapshot(&self) -> Vec<FrameEntry> {
        self.entries.iter().cloned().collect()
    }
}

/// The frame recording, shared by the broker thread (which writes to it) and
/// the `record_frames` command (which starts and stops it).
#[derive(Clone, Default)]
pub struct Recorder(Arc<Mutex<Option<FrameLog>>>);

impl Recorder {
    /// Starts (keeping a recording already running) or stops recording;
    /// returns the file being written, `None` once stopped.
    pub fn set(&self, dir: &Path, on: bool) -> io::Result<Option<PathBuf>> {
        let mut log = self
            .0
            .lock()
            .map_err(|_| io::Error::other("recorder poisoned"))?;
        if !on {
            *log = None;
            return Ok(None);
        }
        if log.is_none() {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |d| d.as_secs());
            *log = Some(FrameLog::create_in(dir, now)?);
        }
        Ok(log.as_ref().map(|l| l.path().to_owned()))
    }

    /// The file being written, if recording.
    pub fn path(&self) -> Option<PathBuf> {
        self.0.lock().ok()?.as_ref().map(|l| l.path().to_owned())
    }

    fn with(&self, f: impl FnOnce(&mut FrameLog)) {
        if let Ok(mut log) = self.0.lock() {
            if let Some(log) = log.as_mut() {
                f(log);
            }
        }
    }
}

/// Publishes a frame snapshot (the `"frames"` event).
pub type EmitFrames<'a> = Box<dyn Fn(&[FrameEntry]) + 'a>;
/// Publishes the recording file or `None` (the `"recording"` event).
pub type EmitRecording<'a> = Box<dyn Fn(Option<&Path>) + 'a>;

/// Everything that watches the wire: the ring, the recording, and whether
/// the window is streaming frames. Outlives each connection.
pub struct FrameTap<'a> {
    ring: FrameRing,
    recorder: Recorder,
    /// Where `RecordFrames(true)` creates its file.
    log_dir: PathBuf,
    watching: bool,
    /// A frame arrived since the last `"frames"` emit.
    dirty: bool,
    /// When `flush` last emitted, on the broker's clock.
    last_emit_ms: Option<u64>,
    emit_frames: EmitFrames<'a>,
    emit_recording: EmitRecording<'a>,
}

impl<'a> FrameTap<'a> {
    pub fn new(
        log_dir: PathBuf,
        recorder: Recorder,
        emit_frames: EmitFrames<'a>,
        emit_recording: EmitRecording<'a>,
    ) -> FrameTap<'a> {
        FrameTap {
            ring: FrameRing::new(RING_CAPACITY),
            recorder,
            log_dir,
            watching: false,
            dirty: false,
            last_emit_ms: None,
            emit_frames,
            emit_recording,
        }
    }

    pub fn sent(&mut self, t_ms: u64, frame: &[u8]) {
        self.recorder.with(|log| log.sent(t_ms, frame));
        self.ring.push_sent(t_ms, frame);
        self.dirty = true;
    }

    pub fn received(&mut self, t_ms: u64, chunk: &[u8]) {
        self.recorder.with(|log| log.received(t_ms, chunk));
        self.ring.push_received(t_ms, chunk);
        self.dirty = true;
    }

    /// A session log line (recording only).
    pub fn note(&mut self, t_ms: u64, msg: &str) {
        self.recorder.with(|log| log.note(t_ms, msg));
    }

    /// Called when a new connection starts.
    pub fn new_link(&mut self) {
        self.ring.reset_partial();
    }

    /// Starts or stops streaming; starting emits the current snapshot and
    /// the recording state at once (a window that just opened knows neither).
    pub fn watch(&mut self, on: bool) {
        self.watching = on;
        if on {
            (self.emit_frames)(&self.ring.snapshot());
            self.dirty = false;
            (self.emit_recording)(self.recorder.path().as_deref());
        }
    }

    /// Starts or stops recording and emits `"recording"`.
    pub fn record(&mut self, on: bool) {
        match self.recorder.set(&self.log_dir, on) {
            Ok(path) => {
                if let Some(path) = &path {
                    eprintln!("hairline: frame log at {}", path.display());
                }
                (self.emit_recording)(path.as_deref());
            }
            Err(e) => {
                eprintln!("hairline: frame log not started ({e})");
                (self.emit_recording)(self.recorder.path().as_deref());
            }
        }
    }

    /// Emits one snapshot if frames arrived since the last one and the window
    /// watches, at most every `FRAMES_EVERY_MS`: frames that arrive sooner stay
    /// pending for a later call. The broker calls this every loop (10 ms).
    pub fn flush(&mut self, now_ms: u64) {
        if !self.watching {
            self.dirty = false;
            return;
        }
        let due = self
            .last_emit_ms
            .is_none_or(|t| now_ms >= t + FRAMES_EVERY_MS);
        if self.dirty && due {
            (self.emit_frames)(&self.ring.snapshot());
            self.dirty = false;
            self.last_emit_ms = Some(now_ms);
        }
    }

    /// Emits any pending snapshot now, ignoring the throttle (the link ended,
    /// so no further `flush` is coming).
    pub fn flush_pending(&mut self) {
        if self.watching && self.dirty {
            (self.emit_frames)(&self.ring.snapshot());
        }
        self.dirty = false;
    }
}
