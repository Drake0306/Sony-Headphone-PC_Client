pub(crate) const START: u8 = 0x3E;
pub(crate) const END: u8 = 0x3C;
pub(crate) const ESCAPE: u8 = 0x3D;
/// Escaped bytes are transmitted as `ESCAPE` followed by `byte & 0xEF`.
pub(crate) const ESCAPE_MASK: u8 = 0xEF;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameType {
    Ack,
    Command1,
    Command2,
    Other(u8),
}

impl FrameType {
    pub fn to_u8(self) -> u8 {
        match self {
            FrameType::Ack => 0x01,
            FrameType::Command1 => 0x0C,
            FrameType::Command2 => 0x0E,
            FrameType::Other(b) => b,
        }
    }

    pub fn from_u8(b: u8) -> Self {
        match b {
            0x01 => FrameType::Ack,
            0x0C => FrameType::Command1,
            0x0E => FrameType::Command2,
            other => FrameType::Other(other),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    pub frame_type: FrameType,
    pub seq: u8,
    pub payload: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FrameError {
    BadChecksum { expected: u8, got: u8 },
    BadLength,
    Truncated,
}

pub(crate) fn checksum(bytes: &[u8]) -> u8 {
    bytes.iter().fold(0u8, |acc, b| acc.wrapping_add(*b))
}

fn push_escaped(out: &mut Vec<u8>, b: u8) {
    if matches!(b, START | END | ESCAPE) {
        out.push(ESCAPE);
        out.push(b & ESCAPE_MASK);
    } else {
        out.push(b);
    }
}

pub fn encode(frame: &Frame) -> Vec<u8> {
    let mut inner = Vec::with_capacity(frame.payload.len() + 7);
    inner.push(frame.frame_type.to_u8());
    inner.push(frame.seq);
    inner.extend_from_slice(&(frame.payload.len() as u32).to_be_bytes());
    inner.extend_from_slice(&frame.payload);
    let sum = checksum(&inner);
    inner.push(sum);

    let mut out = Vec::with_capacity(inner.len() + 2);
    out.push(START);
    for b in inner {
        push_escaped(&mut out, b);
    }
    out.push(END);
    out
}

pub fn ack_for(received_seq: u8) -> Frame {
    Frame {
        frame_type: FrameType::Ack,
        seq: received_seq ^ 1,
        payload: Vec::new(),
    }
}
