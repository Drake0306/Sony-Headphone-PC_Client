//! The broker loop against the fake device, with a channel standing in for
//! the Tauri event emitter.

use std::sync::mpsc::{channel, Receiver};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use hairline_lib::broker::{next_backoff, parse_raw, run_broker, run_broker_logged, BrokerCmd};
use hairline_lib::frame_log::{file_name, FrameLog};
use mdr_proto::Table;
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

#[test]
fn reconnect_backoff_doubles_from_five_to_sixty_seconds() {
    let mut wait = 0;
    let seq: Vec<u64> = (0..7)
        .map(|_| {
            wait = next_backoff(wait);
            wait
        })
        .collect();
    assert_eq!(seq, [5, 10, 20, 40, 60, 60, 60]);
}

#[test]
fn raw_command_parses_table_and_hex() {
    let want = Ok(BrokerCmd::Raw {
        table: Table::T1,
        bytes: vec![0x22, 0x00],
    });
    assert_eq!(parse_raw("t1", "22 00"), want);
    assert_eq!(parse_raw("T1", "2200"), want);
    assert_eq!(
        parse_raw("t2", "06:00"),
        Ok(BrokerCmd::Raw {
            table: Table::T2,
            bytes: vec![0x06, 0x00],
        })
    );
    assert!(parse_raw("t3", "22 00").is_err());
    assert!(parse_raw("t1", "2").is_err());
    assert!(parse_raw("t1", "zz").is_err());
    assert!(parse_raw("t1", "").is_err());
}

#[test]
fn frame_log_file_is_named_by_utc_time() {
    assert_eq!(file_name(0), "1970-01-01-000000.log");
    assert_eq!(file_name(1_794_000_000), "2026-11-06-212000.log");
}

#[test]
fn broker_writes_frames_and_logs_to_the_frame_log() {
    let dir = std::env::temp_dir().join(format!("hairline-frame-log-{}", std::process::id()));
    let mut log = FrameLog::create_in(&dir, 1_794_000_000).unwrap();
    let path = log.path().to_owned();
    let (tx, rx) = channel();
    let (etx, erx) = channel();
    let t = Box::new(FakeDevice::ult_wear());
    let handle = std::thread::spawn(move || {
        run_broker_logged(
            t,
            rx,
            |s| etx.send(s.clone()).unwrap(),
            monotonic_ms,
            Some(&mut log),
        )
    });
    wait_for(&erx, |s| s.connected);
    tx.send(BrokerCmd::Raw {
        table: Table::T1,
        bytes: vec![0x22, 0x00],
    })
    .unwrap();
    // Give the raw request time to be answered before disconnecting.
    std::thread::sleep(Duration::from_millis(300));
    tx.send(BrokerCmd::Disconnect).unwrap();
    assert!(handle.join().unwrap().is_ok());
    let text = std::fs::read_to_string(&path).unwrap();
    std::fs::remove_dir_all(&dir).unwrap();
    let lines: Vec<Vec<&str>> = text.lines().map(|l| l.splitn(3, ' ').collect()).collect();
    for line in &lines {
        assert!(line[0].parse::<u64>().is_ok(), "{line:?}");
    }
    let frame = |dir: &str| {
        lines
            .iter()
            .any(|l| l[1] == dir && !l[2].is_empty() && l[2].chars().all(|c| c.is_ascii_hexdigit()))
    };
    assert!(frame(">"), "no outgoing frame in {text}");
    assert!(frame("<"), "no incoming frame in {text}");
    assert!(
        lines
            .iter()
            .any(|l| l[1] == "#" && l[2].starts_with("raw reply T1 23 00")),
        "no raw reply in {text}"
    );
}
