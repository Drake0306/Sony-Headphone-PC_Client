//! Connection handshake: a fixed sequence of reads, each waiting for its reply.

use std::collections::VecDeque;

use mdr_proto::opcode::{t1, t2};
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
    /// T1 capabilities, kept from the T1 support-function reply until the T2
    /// one (or its timeout) schedules the capability reads.
    t1_caps: Option<Vec<u8>>,
}

/// Initial-state reads, in order, filtered to the declared capabilities of
/// each request's table.
fn capability_reads<'a>(t1: &'a [u8], t2: &'a [u8]) -> impl Iterator<Item = Request> + 'a {
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
    reads.extend([
        request::auto_power_off_get(),
        request::volume_get(),
        request::dsee_status_get(),
        request::dsee_auto_off_get(),
        request::connection_mode_get(),
        request::wear_pause_get(),
        request::voice_assistant_get(),
        request::quick_access_get(),
        request::voice_guide_volume_get(),
        request::playback_status_get(),
    ]);
    reads.into_iter().filter(move |r| {
        let caps = match r.table {
            Table::T1 => t1,
            Table::T2 => t2,
        };
        r.capability.is_none_or(|c| caps.contains(&c))
    })
}

fn is_support_reply(a: &Awaited, table: Table) -> bool {
    let op = match table {
        Table::T1 => t1::CONNECT_RET_SUPPORT_FUNCTION,
        Table::T2 => t2::CONNECT_RET_SUPPORT_FUNCTION,
    };
    (a.table, a.opcode, a.sub) == (table, op, 0x00)
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

    /// Offers a received payload. The capability lists must already reflect it,
    /// so that the support-function replies can schedule the capability reads.
    pub fn on_reply(
        &mut self,
        table: Table,
        payload: &[u8],
        t1_caps: &[u8],
        t2_caps: &[u8],
    ) -> Step {
        let Some(a) = self.awaiting.filter(|a| {
            matches!(payload, [op, sub, ..] if a.table == table && a.opcode == *op && a.sub == *sub)
        }) else {
            return Step::NotMine;
        };
        if is_support_reply(&a, Table::T1) {
            self.t1_caps = Some(t1_caps.to_vec());
        } else if is_support_reply(&a, Table::T2) {
            self.schedule_reads(t2_caps);
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
        let a = self.awaiting?;
        if is_support_reply(&a, Table::T2) {
            // No T2 list: read only what T1 declares.
            self.schedule_reads(&[]);
        }
        Some((a.group, self.advance()))
    }

    /// Queues the capability reads once both support-function lists are known.
    /// Without a T1 reply (it timed out) nothing is declared, so nothing is read.
    fn schedule_reads(&mut self, t2_caps: &[u8]) {
        if let Some(t1_caps) = self.t1_caps.take() {
            self.steps.extend(capability_reads(&t1_caps, t2_caps));
        }
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
