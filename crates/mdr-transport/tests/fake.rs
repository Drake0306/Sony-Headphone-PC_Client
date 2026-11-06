use std::time::Duration;

use mdr_proto::{Charging, NcAsmState, NcMode, UltLevel};
use mdr_session::{Event, Session, Status, REPLY_TIMEOUT_MS};
use mdr_transport::{FakeDevice, Transport};

/// Writes everything the session has queued, then feeds it at most one frame from the device.
/// Returns false when the device had nothing to send.
fn pump_once(s: &mut Session, t: &mut FakeDevice, now: u64) -> (bool, Vec<Event>) {
    while let Some(out) = s.poll_output() {
        t.write(&out).unwrap();
    }
    match t.recv_timeout(Duration::from_millis(10)).unwrap() {
        Some(bytes) => (true, s.handle_input(&bytes, now)),
        None => (false, Vec::new()),
    }
}

fn connected(t: &mut FakeDevice) -> (Session, u64) {
    let mut s = Session::new(0);
    s.start();
    let mut now = 0;
    for _ in 0..200 {
        now += 5;
        if pump_once(&mut s, t, now).1.contains(&Event::Connected) {
            return (s, now);
        }
    }
    panic!("handshake did not complete");
}

#[test]
fn fake_device_completes_handshake_through_session() {
    let mut t = FakeDevice::ult_wear();
    let mut s = Session::new(0);
    s.start();
    let mut now = 0;
    let mut connected = false;
    for _ in 0..200 {
        while let Some(out) = s.poll_output() {
            t.write(&out).unwrap();
        }
        if let Some(bytes) = t.recv_timeout(Duration::from_millis(10)).unwrap() {
            now += 5;
            if s.handle_input(&bytes, now).contains(&Event::Connected) {
                connected = true;
                break;
            }
        }
    }
    assert!(connected);
    assert_eq!(s.state().model.as_deref(), Some("ULT WEAR"));
    assert_eq!(s.state().battery.value, Some((67, Charging::NotCharging)));
}

#[test]
fn fake_device_echoes_nc_set_as_ntfy() {
    let mut t = FakeDevice::ult_wear();
    let (mut s, mut now) = connected(&mut t);
    s.set_nc(NcAsmState {
        mode: NcMode::Off,
        level: 10,
        voice_focus: false,
    })
    .unwrap();
    assert!(matches!(s.state().nc.status, Status::Pending { .. }));
    for _ in 0..50 {
        now += 5;
        pump_once(&mut s, &mut t, now);
        if s.state().nc.status == Status::Confirmed {
            break;
        }
    }
    assert_eq!(s.state().nc.status, Status::Confirmed);
    assert_eq!(s.state().nc.value.map(|nc| nc.mode), Some(NcMode::Off));
}

#[test]
fn silent_opcode_leads_to_refused() {
    let mut t = FakeDevice::ult_wear();
    let (mut s, mut now) = connected(&mut t);
    t.silent_on(0x58);
    s.set_ult(UltLevel::Ult2).unwrap();
    // Deliver the SET and the device's ACK; no reply follows.
    for _ in 0..20 {
        now += 5;
        if !pump_once(&mut s, &mut t, now).0 {
            break;
        }
    }
    assert!(matches!(s.state().eq_ult.status, Status::Pending { .. }));
    now += REPLY_TIMEOUT_MS + 1;
    s.handle_timeout(now);
    assert_eq!(
        s.state().eq_ult.status,
        Status::Refused {
            reason: "no reply".into()
        }
    );
}

#[test]
fn dropped_link_fails_reads_and_writes() {
    let mut t = FakeDevice::ult_wear();
    t.drop_link_after(1);
    let mut s = Session::new(0);
    s.start();
    let first = s.poll_output().unwrap();
    t.write(&first).unwrap();
    let err = t.recv_timeout(Duration::from_millis(10)).unwrap_err();
    assert_eq!(err.kind(), std::io::ErrorKind::BrokenPipe);
    assert_eq!(
        t.write(&first).unwrap_err().kind(),
        std::io::ErrorKind::BrokenPipe
    );
}
