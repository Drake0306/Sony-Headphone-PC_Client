//! A scripted ULT WEAR that answers the session handshake and echoes SETs.

use std::collections::VecDeque;
use std::io;
use std::time::{Duration, Instant};

use mdr_frame::{ack_for, encode, Decoder, Frame, FrameType};

use crate::Transport;

/// Elapsed time after the first write at which the battery NTFY is sent.
const BATTERY_NTFY_AFTER: Duration = Duration::from_secs(30);
const BATTERY_NTFY: &[u8] = &[0x25, 0x00, 0x42, 0x00];
/// RFCOMM channel the ULT WEAR advertises for the v2 service.
const CHANNEL: u8 = 18;

/// T1 support functions the fake declares: battery, power off, EQ/ULT, NC/ASM,
/// codec, multipoint, then DSEE, auto power-off, playback, upscaling,
/// connection mode, wear pause, voice assistant, quick access, touch panel and
/// sidetone.
const T1_CAPS: &[u8] = &[
    0x20, 0x23, 0x53, 0x6B, 0x12, 0xD1, 0x13, 0x25, 0xA1, 0xE2, 0xE3, 0xF1, 0xF4, 0xFD, 0xD2, 0xD3,
];

/// T2 support functions, as the ULT WEAR declares them (voice guidance first).
const T2_CAPS: &[u8] = &[0x42, 0x50, 0xF2];

/// EQ preset ids the ULT WEAR reports (docs/hardware-validation.md).
const PRESETS: [u8; 12] = [
    0x00, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0xA0, 0xA1, 0xA2,
];

/// A support-function reply (`07 00 <n> (<id> <priority>)×n`).
fn support_reply(caps: &[u8]) -> Vec<u8> {
    let mut r = vec![0x07, 0x00, caps.len() as u8];
    for &id in caps {
        r.extend_from_slice(&[id, 0x01]);
    }
    r
}

/// Reply for a request, or `None` if the fake has no answer.
fn reply(frame_type: FrameType, req: &[u8], t1_caps: &[u8]) -> Option<Vec<u8>> {
    let head = req.get(..2)?;
    let reply = match (frame_type, head) {
        (FrameType::Command1, [0x00, 0x00]) => vec![0x01, 0x00, 0x03, 0x00, 0x20, 0x02, 0x00, 0x00],
        (FrameType::Command1, [0x06, 0x00]) => support_reply(t1_caps),
        (FrameType::Command2, [0x06, 0x00]) => support_reply(T2_CAPS),
        (FrameType::Command2, [0x46, 0x20]) => vec![0x47, 0x20, 0x02],
        // T2 voice guidance volume SET, confirmed by its NTFY.
        (FrameType::Command2, [0x48, 0x20]) => echo(req),
        (_, [0x04, 0x01]) => [&[0x05, 0x01, 0x08][..], b"ULT WEAR"].concat(),
        (_, [0x04, 0x02]) => [&[0x05, 0x02, 0x05][..], b"2.1.0"].concat(),
        (_, [0x04, 0x03]) => vec![0x05, 0x03, 0x00, 0x08],
        (_, [0x22, 0x00]) => vec![0x23, 0x00, 0x43, 0x00],
        (_, [0x66, 0x17]) => vec![0x67, 0x17, 0x01, 0x01, 0x01, 0x01, 0x0E],
        // As the ULT WEAR answers: 6 bands, 21 steps, ULT2 step 10, then
        // twelve preset ids with empty names.
        (_, [0x50, 0x03]) => {
            let mut r = vec![0x51, 0x03, 0x06, 0x15, 0x0A, PRESETS.len() as u8];
            r.extend(PRESETS.iter().flat_map(|&id| [id, 0x00]));
            r
        }
        (_, [0x56, 0x03]) => vec![
            0x57, 0x03, 0x00, 0x01, 0x06, 0x0A, 0x0A, 0x0A, 0x0A, 0x0A, 0x0A,
        ],
        (_, [0x12, 0x02]) => vec![0x13, 0x02, 0x10],
        (_, [0xD0, 0xD1]) => [&[0xD1, 0xD1, 0x00, 0x00, 0x0A][..], b"MULTIPOINT", &[0x00]].concat(),
        (_, [0xD6, 0xD1]) => vec![0xD7, 0xD1, 0x00, 0x00],
        (_, [0xD0, 0xD2]) => [
            &[0xD1, 0xD2, 0x00, 0x00, 0x13][..],
            b"TOUCH_PANEL_SETTING",
            &[0x00],
        ]
        .concat(),
        (_, [0xD6, 0xD2]) => vec![0xD7, 0xD2, 0x00, 0x01],
        (_, [0xD0, 0xD3]) => [
            &[0xD1, 0xD3, 0x00, 0x00, 0x10][..],
            b"SIDETONE_SETTING",
            &[0x18],
            b"SIDETONE_SETTING_SUMMARY",
        ]
        .concat(),
        (_, [0xD6, 0xD3]) => vec![0xD7, 0xD3, 0x00, 0x00],
        (_, [0x26, 0x05]) => vec![0x27, 0x05, 0x10, 0x11],
        (_, [0xA6, 0x20]) => vec![0xA7, 0x20, 0x1C],
        (_, [0x12, 0x03]) => vec![0x13, 0x03, 0x01, 0x01],
        (_, [0xE6, 0x01]) => vec![0xE7, 0x01, 0x01],
        (_, [0xE6, 0x02]) => vec![0xE7, 0x02, 0x00],
        (_, [0xF6, 0x01]) => vec![0xF7, 0x01, 0x01],
        (_, [0xF6, 0x04]) => vec![0xF7, 0x04, 0xFF],
        (_, [0xF6, 0x0D]) => vec![0xF7, 0x0D, 0x02, 0x00, 0x00],
        (_, [0xA2, 0x01]) => vec![0xA3, 0x01, 0x00, 0x02, 0x00],
        // SETs are confirmed by a NTFY (the SET opcode + 1) carrying the new value:
        // NC/ASM, EQ/ULT, general settings, upscaling and connection mode,
        // system params, volume and auto power-off.
        (FrameType::Command1, [0x68 | 0x58 | 0xD8 | 0xE8 | 0xF8 | 0xA8 | 0x28, _]) => echo(req),
        _ => return None,
    };
    Some(reply)
}

/// The NTFY for a SET: same payload, opcode + 1.
fn echo(req: &[u8]) -> Vec<u8> {
    let mut r = req.to_vec();
    r[0] += 1;
    r
}

/// A data frame of the fake's own, sent stop-and-wait.
#[derive(Debug)]
struct InFlight {
    seq: u8,
    acked: bool,
}

/// Scripted headphones implementing `Transport` in memory.
#[derive(Debug)]
pub struct FakeDevice {
    decoder: Decoder,
    /// ACKs for our data frames; never gated.
    acks: VecDeque<Vec<u8>>,
    /// The fake's data frames (type, payload) waiting to go out.
    data: VecDeque<(FrameType, Vec<u8>)>,
    in_flight: Option<InFlight>,
    next_seq: u8,
    silent: Vec<u8>,
    /// T1 support functions declared in the handshake.
    t1_caps: Vec<u8>,
    first_write: Option<Instant>,
    battery_sent: bool,
    received: usize,
    drop_after: Option<usize>,
}

impl FakeDevice {
    pub fn ult_wear() -> FakeDevice {
        FakeDevice {
            decoder: Decoder::new(),
            acks: VecDeque::new(),
            data: VecDeque::new(),
            in_flight: None,
            next_seq: 0,
            silent: Vec::new(),
            t1_caps: T1_CAPS.to_vec(),
            first_write: None,
            battery_sent: false,
            received: 0,
            drop_after: None,
        }
    }

    /// Stops replying (but still ACKs) to requests whose first byte is `opcode`.
    pub fn silent_on(&mut self, opcode: u8) {
        self.silent.push(opcode);
    }

    /// Leaves `id` out of the declared T1 support functions, as a model
    /// without that feature would.
    pub fn without_capability(&mut self, id: u8) {
        self.t1_caps.retain(|&c| c != id);
    }

    /// Breaks the link once `n_frames` incoming frames have been received.
    pub fn drop_link_after(&mut self, n_frames: usize) {
        self.drop_after = Some(n_frames);
    }

    fn link_dropped(&self) -> bool {
        self.drop_after.is_some_and(|n| self.received >= n)
    }

    fn on_frame(&mut self, frame: Frame) {
        match frame.frame_type {
            FrameType::Ack => {
                if let Some(f) = self.in_flight.as_mut().filter(|f| frame.seq == f.seq ^ 1) {
                    f.acked = true;
                }
            }
            FrameType::Command1 | FrameType::Command2 => {
                self.acks.push_back(encode(&ack_for(frame.seq)));
                let silenced = frame
                    .payload
                    .first()
                    .is_some_and(|op| self.silent.contains(op));
                if let Some(r) =
                    reply(frame.frame_type, &frame.payload, &self.t1_caps).filter(|_| !silenced)
                {
                    self.data.push_back((frame.frame_type, r));
                }
            }
            FrameType::Other(_) => {}
        }
    }

    fn queue_battery_ntfy_if_due(&mut self) {
        let due = self
            .first_write
            .is_some_and(|t| t.elapsed() >= BATTERY_NTFY_AFTER);
        if due && !self.battery_sent {
            self.battery_sent = true;
            self.data
                .push_back((FrameType::Command1, BATTERY_NTFY.to_vec()));
        }
    }

    /// Next frame to send: ACKs first, then a data frame once the previous one is ACKed.
    fn next_outgoing(&mut self) -> Option<Vec<u8>> {
        if let Some(ack) = self.acks.pop_front() {
            return Some(ack);
        }
        if self.in_flight.as_ref().is_some_and(|f| !f.acked) {
            return None;
        }
        let (frame_type, payload) = self.data.pop_front()?;
        let seq = self.next_seq;
        self.next_seq ^= 1;
        self.in_flight = Some(InFlight { seq, acked: false });
        Some(encode(&Frame {
            frame_type,
            seq,
            payload,
        }))
    }

    /// Time left until the battery NTFY is due, if it is still to come.
    fn until_battery_ntfy(&self) -> Option<Duration> {
        let first = self.first_write.filter(|_| !self.battery_sent)?;
        Some(BATTERY_NTFY_AFTER.saturating_sub(first.elapsed()))
    }
}

fn broken_pipe() -> io::Error {
    io::Error::new(io::ErrorKind::BrokenPipe, "fake link dropped")
}

impl Transport for FakeDevice {
    fn write(&mut self, bytes: &[u8]) -> io::Result<()> {
        if self.link_dropped() {
            return Err(broken_pipe());
        }
        self.first_write.get_or_insert_with(Instant::now);
        for frame in self.decoder.push(bytes).into_iter().flatten() {
            self.received += 1;
            self.on_frame(frame);
            if self.link_dropped() {
                break;
            }
        }
        Ok(())
    }

    fn recv_timeout(&mut self, timeout: Duration) -> io::Result<Option<Vec<u8>>> {
        if self.link_dropped() {
            return Err(broken_pipe());
        }
        self.queue_battery_ntfy_if_due();
        if let Some(bytes) = self.next_outgoing() {
            return Ok(Some(bytes));
        }
        // Nothing to send now; wait as a real link would, unless the NTFY falls due first.
        match self.until_battery_ntfy().filter(|&left| left <= timeout) {
            Some(left) => {
                std::thread::sleep(left);
                self.queue_battery_ntfy_if_due();
                Ok(self.next_outgoing())
            }
            None => {
                std::thread::sleep(timeout);
                Ok(None)
            }
        }
    }

    fn close(&mut self) {}

    fn channel(&self) -> Option<u8> {
        Some(CHANNEL)
    }
}
