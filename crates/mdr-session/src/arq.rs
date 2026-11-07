//! Stop-and-wait ARQ: one data frame in flight, alternating 0/1 sequence numbers.

use std::collections::VecDeque;

use mdr_frame::{ack_for, encode, Frame, FrameType};
use mdr_proto::{Group, Request, Table};

pub const ACK_TIMEOUT_MS: u64 = 1000;
/// Number of ACK timeouts after which the link is given up.
pub const ACK_RETRIES: u64 = 3;

/// A data frame waiting to go out.
#[derive(Debug)]
struct Outgoing {
    request: Request,
    timer: ReplyTimer,
}

#[derive(Debug)]
struct InFlight {
    bytes: Vec<u8>,
    table: Table,
    payload: Vec<u8>,
    seq: u8,
    group: Group,
    timer: ReplyTimer,
    /// `None` until the frame (or its retransmission) has been handed out.
    sent_at: Option<u64>,
    timeouts: u64,
}

/// Which reply timer to start once the device ACKs a frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ReplyTimer {
    /// No reply is waited for (or a lost one is harmless).
    None,
    /// A user request: an unanswered one is refused and re-queried.
    Request,
    /// A handshake read: an unanswered one is refused and skipped.
    Handshake,
}

/// What the device's ACK settled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Acked {
    pub group: Group,
    pub timer: ReplyTimer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Timeout {
    None,
    Retransmit,
    GiveUp,
}

#[derive(Debug, Default)]
pub(crate) struct Arq {
    acks: VecDeque<Vec<u8>>,
    queue: VecDeque<Outgoing>,
    in_flight: Option<InFlight>,
    next_seq: u8,
    last_rx_seq: Option<u8>,
}

fn frame_type(table: Table) -> FrameType {
    match table {
        Table::T1 => FrameType::Command1,
        Table::T2 => FrameType::Command2,
    }
}

impl Arq {
    pub fn enqueue(&mut self, request: Request, timer: ReplyTimer) {
        self.queue.push_back(Outgoing { request, timer });
    }

    /// Next bytes to write: pending ACKs first, then at most one data frame in flight.
    pub fn poll_output(&mut self, now_ms: u64) -> Option<Vec<u8>> {
        if let Some(ack) = self.acks.pop_front() {
            return Some(ack);
        }
        match &mut self.in_flight {
            Some(f) if f.sent_at.is_none() => {
                f.sent_at = Some(now_ms);
                Some(f.bytes.clone())
            }
            Some(_) => None,
            None => {
                let Outgoing { request, timer } = self.queue.pop_front()?;
                let frame = Frame {
                    frame_type: frame_type(request.table),
                    seq: self.next_seq,
                    payload: request.bytes.clone(),
                };
                let bytes = encode(&frame);
                self.in_flight = Some(InFlight {
                    bytes: bytes.clone(),
                    table: request.table,
                    payload: request.bytes,
                    seq: self.next_seq,
                    group: request.group,
                    timer,
                    sent_at: Some(now_ms),
                    timeouts: 0,
                });
                Some(bytes)
            }
        }
    }

    /// The data frame handed out and not yet ACKed: table, payload, group.
    pub fn in_flight(&self) -> Option<(Table, &[u8], Group)> {
        let f = self.in_flight.as_ref()?;
        Some((f.table, &f.payload, f.group))
    }

    /// Drops the first queued (not yet handed out) `Group::Probe` request
    /// with exactly this payload; true if one was dropped.
    pub fn cancel_queued_probe(&mut self, table: Table, payload: &[u8]) -> bool {
        let found = self.queue.iter().position(|o| {
            o.request.group == Group::Probe
                && o.request.table == table
                && o.request.bytes == payload
        });
        found.and_then(|i| self.queue.remove(i)).is_some()
    }

    /// Handles an ACK from the device. Returns what it settled, if it matched the frame in flight.
    pub fn on_ack(&mut self, seq: u8) -> Option<Acked> {
        // An ACK whose seq does not match the frame in flight is stale; ignore it.
        let f = self
            .in_flight
            .as_ref()
            .filter(|f| f.sent_at.is_some() && seq == f.seq ^ 1)?;
        let acked = Acked {
            group: f.group,
            timer: f.timer,
        };
        self.in_flight = None;
        self.next_seq ^= 1;
        Some(acked)
    }

    /// Queues the ACK for an incoming data frame. Returns false for a repeat of the previous frame.
    pub fn on_data(&mut self, seq: u8) -> bool {
        self.acks.push_back(encode(&ack_for(seq)));
        let fresh = self.last_rx_seq != Some(seq);
        self.last_rx_seq = Some(seq);
        fresh
    }

    pub fn deadline(&self) -> Option<u64> {
        Some(self.in_flight.as_ref()?.sent_at? + ACK_TIMEOUT_MS)
    }

    pub fn on_timeout(&mut self, now_ms: u64) -> Timeout {
        if self.deadline().is_none_or(|d| now_ms < d) {
            return Timeout::None;
        }
        let f = self
            .in_flight
            .as_mut()
            .expect("deadline implies a frame in flight");
        f.timeouts += 1;
        if f.timeouts >= ACK_RETRIES {
            return Timeout::GiveUp;
        }
        f.sent_at = None;
        Timeout::Retransmit
    }
}
