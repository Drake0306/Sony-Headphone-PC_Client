//! The opcode probe: walks the GET catalogue one request at a time and
//! records which reads the headphones answer.
//!
//! One request in flight; a reply is a frame on the same table whose first
//! byte is the opcode + 1 and, when the request carried an inquired type,
//! whose second byte repeats it. No reply within `reply_ms` (1.5 s) of the
//! request reaching the wire counts as silent (the session's ARQ may hold it
//! behind other traffic first; a request that never leaves the queue within
//! `QUEUE_LIMIT_FACTOR` × `reply_ms` is recalled and counted silent). The
//! next request waits `gap_ms` (150 ms). Stop recalls a request still
//! queued; one already on the wire cannot be recalled. Every payload is
//! re-checked with `mdr_proto::probe::may_send` (a libmdr GET, not
//! deny-listed) right before it is handed to `Session::request`, which
//! applies the deny-list again.

use std::collections::VecDeque;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use mdr_proto::function::{self, for_opcode};
use mdr_proto::probe::{may_send, probe_points, ProbePoint, GET_CATALOGUE};
use mdr_proto::{Group, Request, Table};
use mdr_session::{DeviceState, Event, Session, SessionError};
use serde::Serialize;

use crate::device_record;
use crate::settings::{raw_allowed, AppSettings};

/// How long a request may wait for its reply.
pub const REPLY_MS: u64 = 1500;
/// The pause between one request's end and the next request.
pub const GAP_MS: u64 = 150;
/// A request still queued (not on the wire) after this many reply waits is
/// recalled and counted silent.
const QUEUE_LIMIT_FACTOR: u64 = 10;

/// The request in flight.
#[derive(Debug, Clone, Copy)]
struct Current {
    point: ProbePoint,
    /// When it was handed to the session (queued in its ARQ).
    queued_at: u64,
    /// When its frame was first seen on the wire; the reply wait starts here.
    on_wire_at: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum ProbeState {
    Idle,
    Running,
    Done,
}

/// One request and what came of it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProbeResult {
    pub table: Table,
    pub opcode: u8,
    pub inquired: Option<u8>,
    /// The libmdr command name.
    pub name: &'static str,
    /// The support function this read belongs to (`function::for_opcode`).
    pub function: Option<u8>,
    pub function_name: Option<&'static str>,
    /// The reply payload; `None` when the device stayed silent.
    pub reply: Option<Vec<u8>>,
    /// The function is in the device's declared list.
    pub declared: bool,
    /// Answered, maps to a function, and that function is not declared.
    pub proven: bool,
}

/// The `"probe"` event payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProbeEvent {
    pub state: ProbeState,
    pub done: usize,
    pub total: usize,
    pub results: Vec<ProbeResult>,
    /// The results file, once written.
    pub file: Option<String>,
    /// The reply wait and the gap, for the "up to about N minutes" estimate.
    pub reply_ms: u64,
    pub gap_ms: u64,
}

/// The probe may run only with the Advanced panel enabled and the
/// headphones connected.
pub fn probe_allowed(settings: &AppSettings, state: &DeviceState) -> Result<(), String> {
    raw_allowed(settings)?;
    if !state.connected {
        return Err("not connected".into());
    }
    Ok(())
}

/// What `ProbeRunner::tick` did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    /// Nothing changed.
    Waiting,
    /// A result was recorded.
    Progress,
    /// The last result is in, or the link refused the next request.
    Finished,
}

/// The probe's state machine; the broker drives it with the clock.
pub struct ProbeRunner {
    reply_ms: u64,
    gap_ms: u64,
    points: Vec<ProbePoint>,
    queue: VecDeque<ProbePoint>,
    current: Option<Current>,
    /// The next request goes out no earlier than this.
    next_at: u64,
    results: Vec<ProbeResult>,
    state: ProbeState,
    total: usize,
}

impl Default for ProbeRunner {
    fn default() -> Self {
        Self::new()
    }
}

impl ProbeRunner {
    /// The whole catalogue, 1.5 s reply wait, 150 ms gap.
    pub fn new() -> ProbeRunner {
        Self::with_timing(REPLY_MS, GAP_MS)
    }

    /// As `new` with other waits (tests).
    pub fn with_timing(reply_ms: u64, gap_ms: u64) -> ProbeRunner {
        ProbeRunner {
            reply_ms,
            gap_ms,
            points: probe_points(GET_CATALOGUE),
            queue: VecDeque::new(),
            current: None,
            next_at: 0,
            results: Vec::new(),
            state: ProbeState::Idle,
            total: 0,
        }
        .counted()
    }

    /// Sets `total` to the number of requests a run would send.
    fn counted(mut self) -> ProbeRunner {
        self.total = self.sendable().count();
        self
    }

    fn sendable(&self) -> impl Iterator<Item = ProbePoint> + '_ {
        self.points
            .iter()
            .copied()
            .filter(|p| may_send(p.table, &p.bytes()))
    }

    /// Probes `points` instead of the catalogue (tests). Points `may_send`
    /// refuses are dropped here and again before sending.
    pub fn with_points(mut self, points: Vec<ProbePoint>) -> ProbeRunner {
        self.points = points;
        self.counted()
    }

    pub fn state(&self) -> ProbeState {
        self.state
    }

    pub fn is_running(&self) -> bool {
        self.state == ProbeState::Running
    }

    /// Starts over with the full list.
    pub fn start(&mut self, now_ms: u64) {
        self.queue = self.sendable().collect();
        self.total = self.queue.len();
        self.current = None;
        self.next_at = now_ms;
        self.results.clear();
        self.state = ProbeState::Running;
    }

    /// Ends the run, keeping the results so far. A request still queued in
    /// the session is recalled; one already on the wire cannot be.
    pub fn stop(&mut self, session: Option<&mut Session>) {
        if let (Some(c), Some(session)) = (self.current, session) {
            session.cancel_queued_raw(c.point.table, &c.point.bytes());
        }
        self.queue.clear();
        self.current = None;
        self.state = ProbeState::Done;
    }

    /// Notes when the request in flight reaches the wire; call after each
    /// batch of writes, before reading the device.
    pub fn on_wire(&mut self, session: &Session, now_ms: u64) {
        let Some(c) = self.current.as_mut().filter(|c| c.on_wire_at.is_none()) else {
            return;
        };
        let bytes = c.point.bytes();
        if session.in_flight() == Some((c.point.table, &bytes[..], Group::Raw)) {
            c.on_wire_at = Some(now_ms);
        }
    }

    /// Records the reply if `bytes` answers the request in flight.
    pub fn on_payload(
        &mut self,
        table: Table,
        bytes: &[u8],
        state: &DeviceState,
        now_ms: u64,
    ) -> bool {
        let Some(Current { point, .. }) = self.current else {
            return false;
        };
        let answers = point.table == table
            && bytes.first() == Some(&point.opcode.wrapping_add(1))
            && point.inquired.is_none_or(|i| bytes.get(1) == Some(&i));
        if answers {
            self.record(point, Some(bytes.to_vec()), state);
            self.next_at = now_ms + self.gap_ms;
        }
        answers
    }

    /// Times out the request in flight, or sends the next one when due.
    pub fn tick(&mut self, session: &mut Session, now_ms: u64) -> Step {
        if !self.is_running() {
            return Step::Waiting;
        }
        if let Some(c) = self.current {
            let deadline = match c.on_wire_at {
                Some(t) => t + self.reply_ms,
                None => c.queued_at + QUEUE_LIMIT_FACTOR * self.reply_ms,
            };
            if now_ms < deadline {
                return Step::Waiting;
            }
            if c.on_wire_at.is_none() {
                session.cancel_queued_raw(c.point.table, &c.point.bytes());
            }
            self.record(c.point, None, session.state());
            self.next_at = now_ms + self.gap_ms;
            return self.finish_if_empty(Step::Progress);
        }
        if now_ms < self.next_at {
            return Step::Waiting;
        }
        let Some(point) = self.queue.pop_front() else {
            self.state = ProbeState::Done;
            return Step::Finished;
        };
        let bytes = point.bytes();
        // Defence in depth: never anything but a catalogue-style GET.
        if !may_send(point.table, &bytes) {
            self.record(point, None, session.state());
            return self.finish_if_empty(Step::Progress);
        }
        match session.request(Request {
            table: point.table,
            bytes,
            capability: None,
            group: Group::Raw,
        }) {
            Ok(()) => {
                self.current = Some(Current {
                    point,
                    queued_at: now_ms,
                    on_wire_at: None,
                });
                Step::Waiting
            }
            Err(SessionError::NotConnected) => {
                self.stop(Some(session));
                Step::Finished
            }
            Err(_) => {
                self.record(point, None, session.state());
                self.finish_if_empty(Step::Progress)
            }
        }
    }

    fn finish_if_empty(&mut self, step: Step) -> Step {
        if self.queue.is_empty() && self.current.is_none() {
            self.state = ProbeState::Done;
            return Step::Finished;
        }
        step
    }

    fn record(&mut self, point: ProbePoint, reply: Option<Vec<u8>>, state: &DeviceState) {
        self.current = None;
        let function = for_opcode(point.table, point.opcode, point.inquired);
        let declared = function.is_some_and(|f| {
            let list = match point.table {
                Table::T1 => &state.capabilities_t1,
                Table::T2 => &state.capabilities_t2,
            };
            list.contains(&f)
        });
        let proven = reply.is_some() && function.is_some() && !declared;
        self.results.push(ProbeResult {
            table: point.table,
            opcode: point.opcode,
            inquired: point.inquired,
            name: point.name,
            function,
            function_name: function.and_then(|f| function::name(point.table, f)),
            reply,
            declared,
            proven,
        });
    }

    /// The undeclared functions that answered, each once, in result order.
    pub fn proven(&self) -> Vec<(Table, u8)> {
        let mut out = Vec::new();
        for r in self.results.iter().filter(|r| r.proven) {
            if let Some(f) = r.function {
                if !out.contains(&(r.table, f)) {
                    out.push((r.table, f));
                }
            }
        }
        out
    }

    pub fn event(&self, file: Option<String>) -> ProbeEvent {
        ProbeEvent {
            state: self.state,
            done: self.results.len(),
            total: self.total,
            results: self.results.clone(),
            file,
            reply_ms: self.reply_ms,
            gap_ms: self.gap_ms,
        }
    }
}

/// Publishes a `"probe"` event.
pub type EmitProbe<'a> = Box<dyn Fn(&ProbeEvent) + 'a>;

/// The runner plus where its results go. Outlives each connection.
pub struct ProbeHost<'a> {
    runner: ProbeRunner,
    /// Where `probe-<unix secs>.json` is written (`captures/local`).
    out_dir: PathBuf,
    emit: EmitProbe<'a>,
    /// The last run's results file.
    file: Option<String>,
}

impl<'a> ProbeHost<'a> {
    pub fn new(runner: ProbeRunner, out_dir: PathBuf, emit: EmitProbe<'a>) -> ProbeHost<'a> {
        ProbeHost {
            runner,
            out_dir,
            emit,
            file: None,
        }
    }

    /// Notes when the probe's request reaches the wire (after the writes).
    pub fn on_wire(&mut self, session: &Session, now_ms: u64) {
        self.runner.on_wire(session, now_ms);
    }

    /// Re-publishes the current state (for a window that just opened).
    pub fn emit_current(&self) {
        (self.emit)(&self.runner.event(self.file.clone()));
    }

    /// Starts a run if connected and not already running; otherwise logs why not.
    pub fn start(&mut self, session: &Session, now_ms: u64) -> Option<String> {
        if !session.state().connected {
            return Some("probe not started: not connected".into());
        }
        if self.runner.is_running() {
            return Some("probe already running".into());
        }
        self.file = None;
        self.runner.start(now_ms);
        self.emit_current();
        None
    }

    /// One broker iteration: matches replies among `events`, then times out
    /// or sends. Finishing marks, saves and writes the results. Returns true
    /// when functions were marked proven (the device state changed).
    pub fn step(
        &mut self,
        session: &mut Session,
        events: &[Event],
        now_ms: u64,
        address: Option<&str>,
        record_dir: Option<&Path>,
    ) -> bool {
        if !self.runner.is_running() {
            return false;
        }
        let mut progressed = false;
        for event in events {
            if let Event::Payload { table, bytes } = event {
                progressed |= self
                    .runner
                    .on_payload(*table, bytes, session.state(), now_ms);
            }
        }
        match self.runner.tick(session, now_ms) {
            Step::Finished => return self.finish(Some(session), address, record_dir),
            Step::Progress => self.emit_current(),
            Step::Waiting if progressed => self.emit_current(),
            Step::Waiting => {}
        }
        false
    }

    /// Ends a running probe (Stop, or the link going away) with its partial
    /// results; returns true when functions were marked proven.
    pub fn abort(
        &mut self,
        session: Option<&mut Session>,
        address: Option<&str>,
        record_dir: Option<&Path>,
    ) -> bool {
        if !self.runner.is_running() {
            return false;
        }
        let mut session = session;
        self.runner.stop(session.as_deref_mut());
        self.finish(session, address, record_dir)
    }

    fn finish(
        &mut self,
        session: Option<&mut Session>,
        address: Option<&str>,
        record_dir: Option<&Path>,
    ) -> bool {
        let proven = self.runner.proven();
        let mut marked = false;
        if let Some(session) = session {
            for &(table, f) in &proven {
                marked |= !session.state().proven.contains(&(table, f));
                session.mark_proven(table, f);
            }
        }
        if let (Some(dir), Some(address), false) = (record_dir, address, proven.is_empty()) {
            let mut record = device_record::load(dir, address);
            for entry in &proven {
                if !record.proven.contains(entry) {
                    record.proven.push(*entry);
                }
            }
            if let Err(e) = device_record::save(dir, address, &record) {
                eprintln!("hairline: device record not saved ({e})");
            }
        }
        self.file = match write_results(&self.out_dir, address, &self.runner.event(None).results) {
            Ok(path) => Some(path.display().to_string()),
            Err(e) => {
                eprintln!("hairline: probe results not written ({e})");
                None
            }
        };
        self.emit_current();
        marked
    }
}

#[derive(Serialize)]
struct ResultsFile<'a> {
    written_unix_secs: u64,
    address: Option<&'a str>,
    results: &'a [ProbeResult],
}

/// Writes `probe-<unix secs>.json` (a numbered suffix if that exists).
fn write_results(
    dir: &Path,
    address: Option<&str>,
    results: &[ProbeResult],
) -> io::Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let text = serde_json::to_string_pretty(&ResultsFile {
        written_unix_secs: now,
        address,
        results,
    })
    .map_err(io::Error::other)?;
    for n in 1.. {
        let name = if n == 1 {
            format!("probe-{now}.json")
        } else {
            format!("probe-{now}-{n}.json")
        };
        let path = dir.join(name);
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(mut f) => {
                io::Write::write_all(&mut f, text.as_bytes())?;
                return Ok(path);
            }
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
        }
    }
    unreachable!("an unused file name exists")
}
