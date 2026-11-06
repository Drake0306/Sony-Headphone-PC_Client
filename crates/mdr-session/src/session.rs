//! The sans-IO session: the caller feeds bytes and the clock, and writes what
//! `poll_output` hands back.

use mdr_frame::{Decoder, Frame, FrameType};
use mdr_proto::opcode::t1;
use mdr_proto::request::{self, Group, Request};
use mdr_proto::{decode, denied, Message, NcAsmState, Table, UltLevel};

use crate::arq::{Arq, ReplyTimer, Timeout};
use crate::handshake::{Handshake, Step};
use crate::state::{Applied, DeviceState, PairedDevice, Status};

/// How long after the device ACKs a request its reply may take.
pub const REPLY_TIMEOUT_MS: u64 = 1500;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    StateChanged,
    Connected,
    Disconnected,
    Log(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionError {
    /// The device does not declare this support function.
    Unavailable(u8),
    /// The payload is on the deny-list.
    Denied(&'static str),
    NotConnected,
    /// Not enough device state yet to build the request.
    Busy,
}

pub struct Session {
    state: DeviceState,
    arq: Arq,
    handshake: Handshake,
    decoder: Decoder,
    /// Groups whose request was ACKed and whose reply is still due, with the ACK time.
    awaiting_reply: Vec<(Group, u64)>,
    /// A raw request was ACKed; the next device data frame is logged as its reply.
    raw_reply_due: bool,
    last_now_ms: u64,
}

/// The read that re-queries a group after its SET went unanswered.
fn refetch(group: Group) -> Option<Request> {
    match group {
        Group::NcAsm => Some(request::ncasm_get()),
        Group::EqUlt => Some(request::eq_ult_get()),
        Group::GeneralSetting(s) => Some(request::gs_get(s)),
        Group::Volume => Some(request::volume_get()),
        Group::AutoPowerOff => Some(request::auto_power_off_get()),
        Group::Battery => Some(request::battery_get()),
        Group::Dsee => Some(request::dsee_status_get()),
        Group::DseeAutoOff => Some(request::dsee_auto_off_get()),
        Group::ConnectionMode => Some(request::connection_mode_get()),
        Group::WearPause => Some(request::wear_pause_get()),
        Group::VoiceAssistant => Some(request::voice_assistant_get()),
        Group::QuickAccess => Some(request::quick_access_get()),
        Group::VoiceGuide => Some(request::voice_guide_volume_get()),
        Group::Playback => Some(request::playback_status_get()),
        Group::Connect | Group::Codec | Group::Power | Group::Raw => None,
    }
}

fn is_power_off(req: &Request) -> bool {
    req.table == Table::T1 && req.bytes.first() == Some(&t1::POWER_SET_STATUS)
}

/// Bytes as space-separated uppercase hex pairs.
fn hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|b| format!("{b:02X}"))
        .collect::<Vec<_>>()
        .join(" ")
}

impl Session {
    pub fn new(now_ms: u64) -> Session {
        Session {
            state: DeviceState::default(),
            arq: Arq::default(),
            handshake: Handshake::default(),
            decoder: Decoder::new(),
            awaiting_reply: Vec::new(),
            raw_reply_due: false,
            last_now_ms: now_ms,
        }
    }

    /// Queues the handshake. `Event::Connected` follows its last reply.
    pub fn start(&mut self) {
        let first = self.handshake.start();
        self.arq.enqueue(first, ReplyTimer::Handshake);
    }

    pub fn state(&self) -> &DeviceState {
        &self.state
    }

    pub fn handle_input(&mut self, bytes: &[u8], now_ms: u64) -> Vec<Event> {
        self.last_now_ms = now_ms;
        let mut events = Vec::new();
        for result in self.decoder.push(bytes) {
            match result {
                Ok(frame) => self.on_frame(frame, &mut events),
                Err(e) => events.push(Event::Log(format!("frame error: {e:?}"))),
            }
        }
        events
    }

    /// Next bytes to write, respecting one data frame in flight. A data frame
    /// handed out here is stamped with `now_ms` for its ACK deadline.
    pub fn poll_output(&mut self, now_ms: u64) -> Option<Vec<u8>> {
        self.last_now_ms = now_ms;
        self.arq.poll_output(now_ms)
    }

    pub fn handle_timeout(&mut self, now_ms: u64) -> Vec<Event> {
        self.last_now_ms = now_ms;
        if self.arq.on_timeout(now_ms) == Timeout::GiveUp {
            return self.mark_disconnected();
        }
        let mut events = Vec::new();
        if let Some((group, step)) = self.handshake.on_timeout(now_ms) {
            self.refuse(group);
            events.push(Event::Log(format!(
                "handshake read for {group:?} got no reply; skipped"
            )));
            events.push(Event::StateChanged);
            self.follow_handshake(step, &mut events);
        }
        let (expired, waiting) = self
            .awaiting_reply
            .drain(..)
            .partition::<Vec<_>, _>(|&(_, acked)| now_ms >= acked + REPLY_TIMEOUT_MS);
        self.awaiting_reply = waiting;
        if !expired.is_empty() {
            for (group, _) in expired {
                self.refuse(group);
                if let Some(get) = refetch(group) {
                    self.arq.enqueue(get, ReplyTimer::None);
                }
            }
            events.push(Event::StateChanged);
        }
        events
    }

    pub fn next_timeout_ms(&self) -> Option<u64> {
        let replies = self
            .awaiting_reply
            .iter()
            .map(|&(_, acked)| acked + REPLY_TIMEOUT_MS);
        self.arq
            .deadline()
            .into_iter()
            .chain(self.handshake.deadline())
            .chain(replies)
            .min()
    }

    /// Sends a request after the deny-list and capability checks; marks its group Pending.
    pub fn request(&mut self, req: Request) -> Result<(), SessionError> {
        self.submit(req, false)
    }

    /// As `request`, but allows power off (`0x24`) once the user has confirmed it.
    pub fn request_confirmed(&mut self, req: Request) -> Result<(), SessionError> {
        self.submit(req, true)
    }

    pub fn set_nc(&mut self, s: NcAsmState) -> Result<(), SessionError> {
        self.request(request::ncasm_set(&s))
    }

    /// Changes only the ULT level, keeping the device's current preset and bands.
    pub fn set_ult(&mut self, ult: UltLevel) -> Result<(), SessionError> {
        let Some(mut eq) = self
            .state
            .eq_ult
            .value
            .clone()
            .filter(|eq| !eq.bands.is_empty())
        else {
            return Err(SessionError::Busy);
        };
        eq.ult = ult;
        self.request(request::eq_ult_set(&eq))
    }

    pub fn mark_disconnected(&mut self) -> Vec<Event> {
        self.state.mark_disconnected();
        self.arq = Arq::default();
        self.handshake.reset();
        self.decoder = Decoder::new();
        self.awaiting_reply.clear();
        self.raw_reply_due = false;
        vec![Event::Disconnected]
    }

    /// Allows requests for a function the device works with but does not declare.
    pub fn mark_proven(&mut self, table: Table, function: u8) {
        if !self.state.proven.contains(&(table, function)) {
            self.state.proven.push((table, function));
        }
    }

    pub fn set_devices(&mut self, devices: Vec<PairedDevice>) {
        self.state.devices = devices;
    }

    fn submit(&mut self, req: Request, confirmed: bool) -> Result<(), SessionError> {
        if !self.state.connected {
            return Err(SessionError::NotConnected);
        }
        if let Some(reason) = denied(req.table, &req.bytes) {
            if !(confirmed && is_power_off(&req)) {
                return Err(SessionError::Denied(reason));
            }
        }
        if let Some(id) = req.capability.filter(|&id| !self.is_capable(req.table, id)) {
            if let Some(status) = self.state.status_mut(req.group) {
                *status = Status::Unavailable;
            }
            return Err(SessionError::Unavailable(id));
        }
        if let Some(status) = self.state.status_mut(req.group) {
            *status = Status::Pending {
                since_ms: self.last_now_ms,
            };
        }
        let timer = if refetch(req.group).is_some() {
            ReplyTimer::Request
        } else {
            ReplyTimer::None
        };
        self.arq.enqueue(req, timer);
        Ok(())
    }

    fn refuse(&mut self, group: Group) {
        if let Some(status) = self.state.status_mut(group) {
            *status = Status::Refused {
                reason: "no reply".into(),
            };
        }
    }

    fn is_capable(&self, table: Table, id: u8) -> bool {
        let declared = match table {
            Table::T1 => &self.state.capabilities_t1,
            Table::T2 => &self.state.capabilities_t2,
        };
        declared.contains(&id) || self.state.proven.contains(&(table, id))
    }

    fn on_frame(&mut self, frame: Frame, events: &mut Vec<Event>) {
        let table = match frame.frame_type {
            FrameType::Command1 => Table::T1,
            FrameType::Command2 => Table::T2,
            FrameType::Ack => {
                match self.arq.on_ack(frame.seq).map(|a| (a.timer, a.group)) {
                    Some((ReplyTimer::Request, group)) if self.still_pending(group) => {
                        self.awaiting_reply.push((group, self.last_now_ms));
                    }
                    Some((ReplyTimer::Handshake, _)) => self.handshake.on_ack(self.last_now_ms),
                    Some((_, Group::Raw)) => self.raw_reply_due = true,
                    _ => {}
                }
                return;
            }
            FrameType::Other(t) => {
                events.push(Event::Log(format!("ignored frame type {t:#04X}")));
                return;
            }
        };
        if self.arq.on_data(frame.seq) {
            self.dispatch(table, &frame.payload, events);
        }
    }

    /// False once a reply for the group has already arrived (it can beat the ACK).
    fn still_pending(&mut self, group: Group) -> bool {
        self.state
            .status_mut(group)
            .is_none_or(|s| matches!(s, Status::Pending { .. }))
    }

    fn dispatch(&mut self, table: Table, payload: &[u8], events: &mut Vec<Event>) {
        if std::mem::take(&mut self.raw_reply_due) {
            events.push(Event::Log(format!("raw reply {table:?} {}", hex(payload))));
        }
        let (_kind, msg) = decode(table, payload);
        if let Message::Unknown { .. } = msg {
            events.push(Event::Log(format!(
                "unhandled {table:?} payload {payload:02X?}"
            )));
        }
        match self.state.apply(table, msg) {
            Applied::Value(group) => {
                self.awaiting_reply.retain(|&(g, _)| g != group);
                events.push(Event::StateChanged);
            }
            Applied::Info => events.push(Event::StateChanged),
            Applied::Ignored => {}
        }
        if !self.handshake.is_active() {
            return;
        }
        let step = self
            .handshake
            .on_reply(table, payload, &self.state.capabilities_t1);
        self.follow_handshake(step, events);
    }

    fn follow_handshake(&mut self, step: Step, events: &mut Vec<Event>) {
        match step {
            Step::NotMine => {}
            Step::Next(req) => self.arq.enqueue(req, ReplyTimer::Handshake),
            Step::Done => {
                self.state.connected = true;
                events.push(Event::Connected);
            }
        }
    }
}
