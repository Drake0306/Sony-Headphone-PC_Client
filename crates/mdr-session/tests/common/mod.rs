// Each test binary uses a different subset of these helpers.
#![allow(dead_code)]

use mdr_frame::{encode, Decoder, Frame, FrameType};
use mdr_session::Session;

pub fn frame(t: FrameType, seq: u8, p: &[u8]) -> Vec<u8> {
    encode(&Frame { frame_type: t, seq, payload: p.to_vec() })
}

pub fn drain(s: &mut Session) -> Vec<Vec<u8>> {
    std::iter::from_fn(|| s.poll_output()).collect()
}

pub fn decode_one(bytes: &[u8]) -> Frame {
    let mut frames = Decoder::new().push(bytes);
    assert_eq!(frames.len(), 1, "expected exactly one frame");
    frames.remove(0).expect("valid frame")
}

pub fn decode_payload(bytes: &[u8]) -> Vec<u8> {
    decode_one(bytes).payload
}

pub fn seq_of(bytes: &[u8]) -> u8 {
    decode_one(bytes).seq
}

/// Capabilities of the scripted device. Includes `0x23` (power off), which the
/// device declares but which needs no handshake read.
const SCRIPT_CAPS: &[u8] = &[0x20, 0x23, 0x53, 0x6B, 0x12, 0xD1];

/// Runs the handshake against the scripted device in `conversation.rs`.
pub fn connected_session() -> Session {
    connected_session_with_caps(SCRIPT_CAPS)
}

/// Device reply for a handshake request, or `None` if the script has no answer.
pub fn scripted_reply(t: FrameType, req: &[u8], t1_caps: &[u8]) -> Option<Vec<u8>> {
    let reply: Vec<u8> = match (t, req) {
        (FrameType::Command1, [0x00, 0x00]) => vec![0x01, 0x00, 0x03, 0x00, 0x20, 0x02, 0x00, 0x00],
        (FrameType::Command1, [0x06, 0x00]) => {
            let mut r = vec![0x07, 0x00, t1_caps.len() as u8];
            for &id in t1_caps {
                r.extend_from_slice(&[id, 0x01]);
            }
            r
        }
        (FrameType::Command2, [0x06, 0x00]) => vec![0x07, 0x00, 0x01, 0x42, 0x01],
        (_, [0x04, 0x01]) => [&[0x05, 0x01, 0x08][..], b"ULT WEAR"].concat(),
        (_, [0x04, 0x02]) => [&[0x05, 0x02, 0x05][..], b"2.1.0"].concat(),
        (_, [0x22, 0x00]) => vec![0x23, 0x00, 0x43, 0x00],
        (_, [0x66, 0x17]) => vec![0x67, 0x17, 0x01, 0x01, 0x01, 0x01, 0x0E],
        (_, [0x50, 0x03]) => vec![0x51, 0x03, 0x06, 0x15, 0x0A, 0x00],
        (_, [0x56, 0x03]) => vec![0x57, 0x03, 0x00, 0x01, 0x06, 0x0A, 0x0A, 0x0A, 0x0A, 0x0A, 0x0A],
        (_, [0x12, 0x02]) => vec![0x13, 0x02, 0x10],
        (_, [0xD0, 0xD1]) => [&[0xD1, 0xD1, 0x00, 0x00, 0x0A][..], b"MULTIPOINT", &[0x00]].concat(),
        (_, [0xD6, 0xD1]) => vec![0xD7, 0xD1, 0x00, 0x00],
        _ => return None,
    };
    Some(reply)
}

pub fn connected_session_with_caps(t1: &[u8]) -> Session {
    let mut s = Session::new(0);
    s.start();
    let mut t = 0;
    loop {
        let out = drain(&mut s);
        let Some(req) = out.iter().map(|b| decode_one(b)).find(|f| f.frame_type != FrameType::Ack) else {
            break;
        };
        let reply = scripted_reply(req.frame_type, &req.payload, t1)
            .unwrap_or_else(|| panic!("no scripted reply for {:02X?}", req.payload));
        s.handle_input(&frame(FrameType::Ack, 1 - req.seq, &[]), t);
        s.handle_input(&frame(req.frame_type, req.seq, &reply), t + 1);
        t += 10;
    }
    assert!(s.state().connected, "handshake did not complete");
    s
}
