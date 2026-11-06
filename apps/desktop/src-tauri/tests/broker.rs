//! The broker loop against the fake device, with a channel standing in for
//! the Tauri event emitter.

use std::sync::mpsc::{channel, Receiver};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use hairline_lib::broker::{run_broker, BrokerCmd};
use mdr_proto::UltLevel;
use mdr_session::{DeviceState, Status};
use mdr_transport::FakeDevice;

fn monotonic_ms() -> u64 {
    static START: OnceLock<Instant> = OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_millis() as u64
}

/// The first emitted state matching `pred`, or a panic after five seconds.
fn wait_for(rx: &Receiver<DeviceState>, pred: impl Fn(&DeviceState) -> bool) -> DeviceState {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        match rx.recv_timeout(left) {
            Ok(s) if pred(&s) => return s,
            Ok(_) => continue,
            Err(e) => panic!("no matching state emitted: {e}"),
        }
    }
}

#[test]
fn broker_connects_and_emits_state_then_applies_set_ult() {
    let (tx, rx) = channel();
    let (etx, erx) = channel();
    let t = Box::new(FakeDevice::ult_wear());
    std::thread::spawn(move || run_broker(t, rx, |s| etx.send(s.clone()).unwrap(), monotonic_ms));
    let st = wait_for(&erx, |s| s.connected);
    assert_eq!(st.model.as_deref(), Some("ULT WEAR"));
    tx.send(BrokerCmd::SetUlt(UltLevel::Ult2)).unwrap();
    let st = wait_for(&erx, |s| {
        s.eq_ult.value.as_ref().map(|e| e.ult) == Some(UltLevel::Ult2)
            && matches!(s.eq_ult.status, Status::Confirmed)
    });
    assert!(st.connected);
    tx.send(BrokerCmd::Disconnect).unwrap();
}

#[test]
fn broker_returns_ok_on_disconnect() {
    let (tx, rx) = channel();
    let (etx, erx) = channel();
    let t = Box::new(FakeDevice::ult_wear());
    let handle = std::thread::spawn(move || {
        run_broker(t, rx, |s| etx.send(s.clone()).unwrap(), monotonic_ms)
    });
    wait_for(&erx, |s| s.connected);
    tx.send(BrokerCmd::Disconnect).unwrap();
    assert!(handle.join().unwrap().is_ok());
}

#[test]
fn disconnect_marks_all_unavailable() {
    let (tx, rx) = channel();
    let (etx, erx) = channel();
    let mut t = FakeDevice::ult_wear();
    // The handshake takes 26 incoming frames; the Refresh after it crosses 28.
    t.drop_link_after(28);
    let handle = std::thread::spawn(move || {
        run_broker(
            Box::new(t),
            rx,
            |s| etx.send(s.clone()).unwrap(),
            monotonic_ms,
        )
    });
    let st = wait_for(&erx, |s| s.connected);
    assert!(matches!(st.battery.status, Status::Confirmed));
    tx.send(BrokerCmd::Refresh).unwrap();
    let st = wait_for(&erx, |s| !s.connected);
    for status in [
        &st.battery.status,
        &st.nc.status,
        &st.eq_ult.status,
        &st.codec.status,
        &st.multipoint.status,
    ] {
        assert_eq!(status, &Status::Unavailable);
    }
    assert!(handle.join().unwrap().is_err());
}
