mod common;

use common::*;
use mdr_proto::{request, Group, NcAsmState, NcMode, Request, Table};
use mdr_session::*;

#[test]
fn undeclared_capability_is_unavailable() {
    let mut s = connected_session_with_caps(&[0x20]); // battery only
    assert_eq!(s.set_nc(NcAsmState { mode: NcMode::Off, level: 1, voice_focus: false }), Err(SessionError::Unavailable(0x6B)));
    assert!(matches!(s.state().nc.status, Status::Unavailable));
}

#[test]
fn proven_capability_is_allowed() {
    let mut s = connected_session_with_caps(&[0x20]);
    s.mark_proven(Table::T1, 0x6B);
    assert!(s.set_nc(NcAsmState { mode: NcMode::Off, level: 1, voice_focus: false }).is_ok());
}

#[test]
fn deny_list_blocks_and_confirmed_path_lifts_power_off() {
    let mut s = connected_session();
    assert!(matches!(s.request(request::power_off()), Err(SessionError::Denied(_))));
    assert!(s.request_confirmed(request::power_off()).is_ok());
    let mut s2 = connected_session();
    let reboot = Request { table: Table::T1, bytes: vec![0x98, 0x00, 0x16, 0x01], capability: None, group: Group::Power };
    assert!(matches!(s2.request_confirmed(reboot), Err(SessionError::Denied(_))));
}

#[test]
fn requests_before_connected_are_rejected() {
    let mut s = Session::new(0);
    assert_eq!(s.request(request::battery_get()), Err(SessionError::NotConnected));
}
