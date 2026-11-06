//! Connection handshake: a fixed sequence of reads, each waiting for its reply.

use std::collections::VecDeque;

use mdr_proto::opcode::t1;
use mdr_proto::request::{self, Request};
use mdr_proto::{Group, Table};

use crate::session::REPLY_TIMEOUT_MS;

/// Result of offering a received payload to the handshake.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Step {
    /// Not the reply the handshake is waiting for.
    NotMine,
    /// Reply accepted; send this next.
    Next(Request),
    /// Reply accepted and it was the last one.
    Done,
}

/// The reply the current step waits for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Awaited {
    table: Table,
    /// RET opcode: the GET opcode + 1.
    opcode: u8,
    sub: u8,
    group: Group,
    /// When the device ACKed the read; the reply timer runs from here.
    acked_at: Option<u64>,
}

#[derive(Debug, Default)]
pub(crate) struct Handshake {
    steps: VecDeque<Request>,
    awaiting: Option<Awaited>,
}

/// Initial-state reads, in order, filtered to the declared T1 capabilities.
fn capability_reads(caps: &[u8]) -> impl Iterator<Item = Request> + '_ {
    let mut reads = vec![
        request::battery_get(),
        request::ncasm_get(),
        request::eq_ult_capability(),
        request::eq_ult_get(),
        request::codec_get(),
    ];
    for setting in [0xD1, 0xD2, 0xD3] {
        reads.push(request::gs_capability(setting));
        reads.push(request::gs_get(setting));
    }
    reads.push(request::auto_power_off_get());
    reads.push(request::volume_get());
    reads.into_iter().filter(|r| r.capability.is_none_or(|c| caps.contains(&c)))
}

impl Handshake {
    /// Resets the handshake and returns its first request.
    pub fn start(&mut self) -> Request {
        self.steps = VecDeque::from([
            request::support_functions(Table::T1),
            request::support_functions(Table::T2),
            request::device_info(0x01),
            request::device_info(0x02),
            request::device_info(0x03),
        ]);
        self.expect(request::protocol_info())
    }

    pub fn is_active(&self) -> bool {
        self.awaiting.is_some()
    }

    pub fn reset(&mut self) {
        *self = Handshake::default();
    }

    /// Offers a received payload. `t1_caps` must already reflect it, so that the
    /// T1 support-function reply can schedule the capability reads.
    pub fn on_reply(&mut self, table: Table, payload: &[u8], t1_caps: &[u8]) -> Step {
        let Some(a) = self.awaiting.filter(|a| {
            matches!(payload, [op, sub, ..] if a.table == table && a.opcode == *op && a.sub == *sub)
        }) else {
            return Step::NotMine;
        };
        if (a.table, a.opcode, a.sub) == (Table::T1, t1::CONNECT_RET_SUPPORT_FUNCTION, 0x00) {
            self.steps.extend(capability_reads(t1_caps));
        }
        self.advance()
    }

    /// Starts the reply timer for the current read.
    pub fn on_ack(&mut self, now_ms: u64) {
        if let Some(a) = self.awaiting.as_mut().filter(|a| a.acked_at.is_none()) {
            a.acked_at = Some(now_ms);
        }
    }

    pub fn deadline(&self) -> Option<u64> {
        Some(self.awaiting?.acked_at? + REPLY_TIMEOUT_MS)
    }

    /// If the current read's reply is overdue, skips it. Returns its group and what follows.
    pub fn on_timeout(&mut self, now_ms: u64) -> Option<(Group, Step)> {
        if self.deadline().is_none_or(|d| now_ms < d) {
            return None;
        }
        let group = self.awaiting?.group;
        Some((group, self.advance()))
    }

    fn advance(&mut self) -> Step {
        match self.steps.pop_front() {
            Some(next) => Step::Next(self.expect(next)),
            None => {
                self.awaiting = None;
                Step::Done
            }
        }
    }

    fn expect(&mut self, req: Request) -> Request {
        self.awaiting = Some(Awaited {
            table: req.table,
            opcode: req.bytes[0].wrapping_add(1),
            sub: req.bytes[1],
            group: req.group,
            acked_at: None,
        });
        req
    }
}
