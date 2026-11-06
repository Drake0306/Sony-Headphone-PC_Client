mod common;

use common::*;
use mdr_frame::FrameType;
use mdr_proto::{Charging, Codec, NcAsmState, NcMode, UltLevel};
use mdr_session::*;
use mdr_frame::Frame;

#[test]
fn handshake_sends_protocol_info_first_and_waits_for_ack() {
    let mut s = Session::new(0);
    s.start();
    let out = drain(&mut s);
    assert_eq!(out, vec![frame(FrameType::Command1, 0, &[0x00, 0x00])]);
    assert!(s.poll_output().is_none(), "one frame in flight");
    s.handle_input(&frame(FrameType::Ack, 1, &[]), 10);
    s.handle_input(&frame(FrameType::Command1, 0, &[0x01, 0x00, 0x03, 0x00, 0x20, 0x02, 0x00, 0x00]), 11);
    let out = drain(&mut s);
    assert_eq!(out[0], frame(FrameType::Ack, 1, &[]), "device data frames are ACKed immediately");
    assert_eq!(out[1], frame(FrameType::Command1, 1, &[0x06, 0x00]));
}

#[test]
fn full_handshake_reaches_connected_and_reads_initial_state() {
    let mut s = Session::new(0);
    s.start();
    let script: Vec<(&[u8], &[u8])> = vec![ // (expected request, device reply) in order
        (&[0x00, 0x00], &[0x01, 0x00, 0x03, 0x00, 0x20, 0x02, 0x00, 0x00]),
        (&[0x06, 0x00], &[0x07, 0x00, 0x05, 0x20, 0x01, 0x53, 0x01, 0x6B, 0x01, 0x12, 0x01, 0xD1, 0x01]),
        // T2 support functions
        (&[0x06, 0x00], &[0x07, 0x00, 0x01, 0x42, 0x01]),
        (&[0x04, 0x01], &[0x05, 0x01, 0x08, b'U', b'L', b'T', b' ', b'W', b'E', b'A', b'R']),
        (&[0x04, 0x02], &[0x05, 0x02, 0x05, b'2', b'.', b'1', b'.', b'0']),
        (&[0x22, 0x00], &[0x23, 0x00, 0x43, 0x00]),
        (&[0x66, 0x17], &[0x67, 0x17, 0x01, 0x01, 0x01, 0x01, 0x0E]),
        (&[0x50, 0x03], &[0x51, 0x03, 0x06, 0x15, 0x0A, 0x00]),
        (&[0x56, 0x03], &[0x57, 0x03, 0x00, 0x01, 0x06, 0x0A, 0x0A, 0x0A, 0x0A, 0x0A, 0x0A]),
        (&[0x12, 0x02], &[0x13, 0x02, 0x10]),
        (&[0xD0, 0xD1], &[0xD1, 0xD1, 0x00, 0x00, 0x0A, b'M', b'U', b'L', b'T', b'I', b'P', b'O', b'I', b'N', b'T', 0x00]),
        (&[0xD6, 0xD1], &[0xD7, 0xD1, 0x00, 0x00]),
    ];
    let mut events = vec![];
    let mut t = 0;
    for (i, (expect, reply)) in script.iter().enumerate() {
        let out = drain(&mut s);
        let req = out.last().unwrap();
        assert_eq!(&decode_payload(req), expect, "request #{i}");
        let seq = seq_of(req);
        events.extend(s.handle_input(&frame(FrameType::Ack, 1 - seq, &[]), t));
        let table = if i == 2 { FrameType::Command2 } else { FrameType::Command1 };
        events.extend(s.handle_input(&frame(table, seq, reply), t + 1));
        t += 10;
    }
    assert!(events.contains(&Event::Connected));
    let st = s.state();
    assert!(st.connected);
    assert_eq!(st.model.as_deref(), Some("ULT WEAR"));
    assert_eq!(st.firmware.as_deref(), Some("2.1.0"));
    assert_eq!(st.capabilities_t1, vec![0x20, 0x53, 0x6B, 0x12, 0xD1]);
    assert_eq!(st.battery.value, Some((67, Charging::NotCharging)));
    assert_eq!(st.nc.value, Some(NcAsmState { mode: NcMode::Ambient, level: 14, voice_focus: true }));
    assert_eq!(st.eq_ult.value.as_ref().unwrap().ult, UltLevel::Ult1);
    assert_eq!(st.codec.value, Some(Codec::Ldac));
    assert_eq!(st.multipoint.value, Some(true));
    assert!(matches!(st.battery.status, Status::Confirmed));
}

#[test]
fn ntfy_without_request_updates_state() {
    let mut s = connected_session(); // helper: runs the script above
    let ev = s.handle_input(&frame(FrameType::Command1, 0, &[0x69, 0x17, 0x01, 0x01, 0x00, 0x00, 0x0E]), 500);
    assert!(ev.contains(&Event::StateChanged));
    assert_eq!(s.state().nc.value.unwrap().mode, NcMode::NoiseCancelling);
    assert!(matches!(s.state().nc.status, Status::Confirmed));
    assert_eq!(drain(&mut s)[0], frame(FrameType::Ack, 1, &[]));
}

#[test]
fn set_nc_marks_pending_then_confirmed_on_ntfy() {
    let mut s = connected_session();
    s.set_nc(NcAsmState { mode: NcMode::Off, level: 14, voice_focus: true }).unwrap();
    assert!(matches!(s.state().nc.status, Status::Pending { .. }));
    let req = drain(&mut s).pop().unwrap();
    assert_eq!(decode_payload(&req), [0x68, 0x17, 0x01, 0x00, 0x00, 0x01, 0x0E]);
    let seq = seq_of(&req);
    s.handle_input(&frame(FrameType::Ack, 1 - seq, &[]), 600);
    s.handle_input(&frame(FrameType::Command1, seq, &[0x69, 0x17, 0x01, 0x00, 0x00, 0x01, 0x0E]), 601);
    assert!(matches!(s.state().nc.status, Status::Confirmed));
    assert_eq!(s.state().nc.value.unwrap().mode, NcMode::Off);
}

#[test]
fn set_ult_is_read_modify_write() {
    let mut s = connected_session();
    s.set_ult(UltLevel::Ult2).unwrap();
    let req = drain(&mut s).pop().unwrap();
    assert_eq!(decode_payload(&req), [0x58, 0x03, 0x00, 0x02, 0x06, 0x0A, 0x0A, 0x0A, 0x0A, 0x0A, 0x0A]);
}

#[test]
fn set_without_reply_becomes_refused_and_requeries() {
    let mut s = connected_session();
    s.set_ult(UltLevel::Off).unwrap();
    let req = drain(&mut s).pop().unwrap();
    let seq = seq_of(&req);
    s.handle_input(&frame(FrameType::Ack, 1 - seq, &[]), 1000);
    assert_eq!(s.next_timeout_ms(), Some(1000 + REPLY_TIMEOUT_MS));
    let ev = s.handle_timeout(2500);
    assert!(ev.contains(&Event::StateChanged));
    assert!(matches!(&s.state().eq_ult.status, Status::Refused { reason } if reason == "no reply"));
    assert_eq!(decode_payload(&drain(&mut s).pop().unwrap()), [0x56, 0x03]);
}

#[test]
fn unacked_frame_is_retransmitted_then_gives_up() {
    let mut s = Session::new(0);
    s.start();
    let first = drain(&mut s).pop().unwrap();
    for n in 1..=ACK_RETRIES {
        let ev = s.handle_timeout(n * ACK_TIMEOUT_MS);
        if n < ACK_RETRIES { assert_eq!(drain(&mut s).pop().unwrap(), first); } else { assert!(ev.contains(&Event::Disconnected)); }
    }
}

#[test]
fn disconnect_marks_all_unavailable() {
    let mut s = connected_session();
    let ev = s.mark_disconnected();
    assert!(ev.contains(&Event::Disconnected));
    assert!(!s.state().connected);
    assert!(matches!(s.state().battery.status, Status::Unavailable));
    assert!(matches!(s.state().nc.status, Status::Unavailable));
}

/// Next data frame the session hands out, draining any ACKs before it.
fn next_request(s: &mut Session) -> Option<Frame> {
    drain(s).iter().map(|b| decode_one(b)).find(|f| f.frame_type != FrameType::Ack)
}

#[test]
fn handshake_read_without_reply_continues() {
    let caps = [0x20, 0x53, 0x6B, 0x12, 0xD1];
    let mut s = Session::new(0);
    s.start();
    let (mut t, mut dev_seq, mut events) = (0, 0, vec![]);
    while let Some(req) = next_request(&mut s) {
        events.extend(s.handle_input(&frame(FrameType::Ack, 1 - req.seq, &[]), t));
        if req.payload == [0x66, 0x17] {
            events.extend(s.handle_timeout(t + 1600));
            assert!(matches!(&s.state().nc.status, Status::Refused { reason } if reason == "no reply"));
            let next = next_request(&mut s).expect("handshake continues");
            assert_eq!(next.payload, [0x50, 0x03]);
            t += 1610;
            events.extend(s.handle_input(&frame(FrameType::Ack, 1 - next.seq, &[]), t));
            let reply = scripted_reply(next.frame_type, &next.payload, &caps).unwrap();
            events.extend(s.handle_input(&frame(next.frame_type, dev_seq, &reply), t + 1));
        } else {
            let reply = scripted_reply(req.frame_type, &req.payload, &caps).unwrap();
            events.extend(s.handle_input(&frame(req.frame_type, dev_seq, &reply), t + 1));
        }
        dev_seq ^= 1;
        t += 10;
    }
    assert!(events.contains(&Event::Connected));
    assert!(s.state().connected);
    assert!(matches!(&s.state().nc.status, Status::Refused { .. }));
}
