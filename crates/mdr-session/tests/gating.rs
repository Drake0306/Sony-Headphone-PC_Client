mod common;

use common::*;
use mdr_proto::{request, Group, NcAsmState, NcMode, Request, Table};
use mdr_session::*;

#[test]
fn undeclared_capability_is_unavailable() {
    let mut s = connected_session_with_caps(&[0x20]); // battery only
    assert_eq!(
        s.set_nc(NcAsmState {
            mode: NcMode::Off,
            level: 1,
            voice_focus: false
        }),
        Err(SessionError::Unavailable(0x6B))
    );
    assert!(matches!(s.state().nc.status, Status::Unavailable));
}

#[test]
fn proven_capability_is_allowed() {
    let mut s = connected_session_with_caps(&[0x20]);
    s.mark_proven(Table::T1, 0x6B);
    assert!(s
        .set_nc(NcAsmState {
            mode: NcMode::Off,
            level: 1,
            voice_focus: false
        })
        .is_ok());
}

#[test]
fn deny_list_blocks_and_confirmed_path_lifts_power_off() {
    let mut s = connected_session();
    assert!(matches!(
        s.request(request::power_off()),
        Err(SessionError::Denied(_))
    ));
    assert!(s.request_confirmed(request::power_off()).is_ok());
    let mut s2 = connected_session();
    let reboot = Request {
        table: Table::T1,
        bytes: vec![0x98, 0x00, 0x16, 0x01],
        capability: None,
        group: Group::Power,
    };
    assert!(matches!(
        s2.request_confirmed(reboot),
        Err(SessionError::Denied(_))
    ));
}

#[test]
fn requests_before_connected_are_rejected() {
    let mut s = Session::new(0);
    assert_eq!(
        s.request(request::battery_get()),
        Err(SessionError::NotConnected)
    );
}

fn raw(bytes: &[u8]) -> Request {
    Request {
        table: Table::T1,
        bytes: bytes.to_vec(),
        capability: None,
        group: Group::Raw,
    }
}

#[test]
fn raw_request_is_still_deny_listed() {
    let mut s = connected_session();
    assert!(matches!(
        s.request(raw(&[0x98, 0x00, 0x16, 0x01])),
        Err(SessionError::Denied(_))
    ));
}

#[test]
fn raw_request_reply_surfaces_as_log() {
    let mut s = connected_session();
    let before = s.state().clone();
    s.request(raw(&[0x22, 0x00])).unwrap();
    assert_eq!(s.state(), &before, "a raw request marks nothing pending");
    let req = drain(&mut s, 500).pop().unwrap();
    assert_eq!(decode_payload(&req), [0x22, 0x00]);
    let seq = seq_of(&req);
    s.handle_input(&frame(mdr_frame::FrameType::Ack, 1 - seq, &[]), 510);
    let ev = s.handle_input(
        &frame(mdr_frame::FrameType::Command1, 1, &[0x23, 0x00, 0x43, 0x00]),
        520,
    );
    assert!(
        ev.contains(&Event::Log("raw reply T1 23 00 43 00".into())),
        "{ev:?}"
    );
    assert_eq!(s.next_timeout_ms(), None, "no reply timer for raw requests");
}

#[test]
fn every_device_data_frame_surfaces_as_a_payload() {
    let mut s = connected_session();
    s.request(Request {
        table: Table::T2,
        ..raw(&[0x32, 0x02])
    })
    .unwrap();
    let req = drain(&mut s, 500).pop().unwrap();
    s.handle_input(
        &frame(mdr_frame::FrameType::Ack, 1 - seq_of(&req), &[]),
        510,
    );
    let ev = s.handle_input(
        &frame(mdr_frame::FrameType::Command2, 1, &[0x33, 0x02, 0x00, 0x01]),
        520,
    );
    assert!(
        ev.contains(&Event::Payload {
            table: Table::T2,
            bytes: vec![0x33, 0x02, 0x00, 0x01],
        }),
        "{ev:?}"
    );
}

#[test]
fn phase2_setters_are_gated() {
    let mut s = connected_session_with_caps(&[0x20]);
    assert_eq!(
        s.set_dsee_auto_off(true),
        Err(SessionError::Unavailable(0xE2))
    );
    assert!(matches!(
        s.state().dsee_auto_off.status,
        Status::Unavailable
    ));
}

#[test]
fn eq_setters_without_the_eq_capability_are_unavailable() {
    let mut s = connected_session_with_caps(&[0x20]);
    let refused = Err(SessionError::Unavailable(0x53));
    assert_eq!(s.set_ult(mdr_proto::UltLevel::Ult2), refused);
    assert_eq!(s.set_eq_preset(0x10), refused);
    assert_eq!(s.set_eq_preset_ult(0x10, mdr_proto::UltLevel::Off), refused);
    assert_eq!(s.set_eq_bands(vec![10; 6]), refused);
    assert!(matches!(s.state().eq_ult.status, Status::Unavailable));
    assert!(drain(&mut s, 10_000).is_empty());
}

#[test]
fn preset_and_ult_go_in_one_request() {
    let mut s = connected_session();
    assert!(drain(&mut s, 10_000).is_empty());
    s.set_eq_preset_ult(0x16, mdr_proto::UltLevel::Ult2)
        .unwrap();
    let out = drain(&mut s, 10_001);
    assert_eq!(out.len(), 1);
    assert_eq!(
        decode_payload(&out[0]),
        [0x58, 0x03, 0x16, 0x02, 0x06, 0x0A, 0x0A, 0x0A, 0x0A, 0x0A, 0x0A]
    );
}

#[test]
fn eq_setters_before_the_handshake_are_not_connected() {
    let mut s = Session::new(0);
    assert_eq!(
        s.set_ult(mdr_proto::UltLevel::Ult2),
        Err(SessionError::NotConnected)
    );
    assert_eq!(s.set_eq_preset(0x10), Err(SessionError::NotConnected));
    assert!(matches!(s.state().eq_ult.status, Status::Unavailable));
}
