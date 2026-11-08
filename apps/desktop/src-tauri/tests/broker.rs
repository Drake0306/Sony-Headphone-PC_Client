//! The broker loop against the fake device, with a channel standing in for
//! the Tauri event emitter.

use std::sync::mpsc::{channel, Receiver};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use mdr_proto::Table;
use mdr_proto::UltLevel;
use mdr_session::{DeviceState, Status};
use mdr_transport::FakeDevice;
use shdc_lib::broker::{
    is_playing, next_backoff, parse_raw, run_broker, run_broker_logged, BrokerCmd, BrokerContext,
};
use shdc_lib::frame_log::file_name;
use shdc_lib::frame_ring::{FrameTap, Recorder};

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
    assert_eq!(st.channel, Some(18));
    assert_eq!(st.address, None);
    tx.send(BrokerCmd::SetUlt(UltLevel::Ult2)).unwrap();
    let st = wait_for(&erx, |s| {
        s.eq_ult.value.as_ref().map(|e| e.ult) == Some(UltLevel::Ult2)
            && matches!(s.eq_ult.status, Status::Confirmed)
    });
    assert!(st.connected);
    tx.send(BrokerCmd::Disconnect).unwrap();
}

#[test]
fn broker_applies_eq_preset_and_dsee() {
    let (tx, rx) = channel();
    let (etx, erx) = channel();
    let t = Box::new(FakeDevice::ult_wear());
    std::thread::spawn(move || run_broker(t, rx, |s| etx.send(s.clone()).unwrap(), monotonic_ms));
    wait_for(&erx, |s| s.connected);
    tx.send(BrokerCmd::SetEqPreset(0x16)).unwrap();
    let st = wait_for(&erx, |s| {
        s.eq_ult.value.as_ref().map(|e| e.preset) == Some(0x16)
            && matches!(s.eq_ult.status, Status::Confirmed)
    });
    assert_eq!(st.eq_ult.value.unwrap().ult, UltLevel::Ult1);
    tx.send(BrokerCmd::SetDseeAutoOff(false)).unwrap();
    wait_for(&erx, |s| {
        s.dsee_auto_off.value == Some(false) && matches!(s.dsee_auto_off.status, Status::Confirmed)
    });
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
    // The handshake sends 27 requests (protocol, two support lists, three
    // device-info reads and 21 capability reads), and the fake receives each
    // request plus the ACK for its reply: 54 incoming frames. The Refresh after
    // it crosses 56 (battery GET and the ACK of its reply).
    t.drop_link_after(56);
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
        confirmed_set: false,
    });
    assert_eq!(parse_raw("t1", "22 00"), want);
    assert_eq!(parse_raw("T1", "2200"), want);
    assert_eq!(
        parse_raw("t2", "06:00"),
        Ok(BrokerCmd::Raw {
            table: Table::T2,
            bytes: vec![0x06, 0x00],
            confirmed_set: false,
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
fn captures_go_under_the_log_dir_else_the_config_dir() {
    use shdc_lib::frame_log::capture_dir;
    use std::path::PathBuf;
    let log = PathBuf::from("/logs/app");
    let config = PathBuf::from("/config/app");
    assert_eq!(
        capture_dir(Some(log.clone()), Some(config.clone())),
        log.join("captures")
    );
    assert_eq!(
        capture_dir(None, Some(config.clone())),
        config.join("captures")
    );
    assert!(capture_dir(None, None).ends_with("captures"));
}

#[test]
fn recording_from_the_tap_writes_into_its_capture_dir() {
    // `SHDC_FRAME_LOG=1` and the Record button both go through `record`.
    let dir = std::env::temp_dir()
        .join(format!("shdc-capture-dir-{}", std::process::id()))
        .join("captures");
    let recorder = Recorder::default();
    let mut tap = FrameTap::new(
        dir.clone(),
        recorder.clone(),
        Box::new(|_| {}),
        Box::new(|_| {}),
    );
    tap.record(true);
    let path = recorder.path().expect("recording");
    assert_eq!(path.parent(), Some(dir.as_path()));
    assert!(path.exists());
    tap.record(false);
    std::fs::remove_dir_all(dir.parent().unwrap()).unwrap();
}

#[test]
fn broker_writes_frames_and_logs_to_the_frame_log() {
    let dir = std::env::temp_dir().join(format!("shdc-frame-log-{}", std::process::id()));
    let recorder = Recorder::default();
    let path = recorder.set(&dir, true).unwrap().expect("a recording path");
    let (tx, rx) = channel();
    let (etx, erx) = channel();
    let t = Box::new(FakeDevice::ult_wear());
    let tap_dir = dir.clone();
    let handle = std::thread::spawn(move || {
        let mut tap = FrameTap::new(tap_dir, recorder, Box::new(|_| {}), Box::new(|_| {}));
        run_broker_logged(
            t,
            rx,
            |s| etx.send(s.clone()).unwrap(),
            monotonic_ms,
            BrokerContext {
                frames: Some(&mut tap),
                ..BrokerContext::default()
            },
        )
    });
    wait_for(&erx, |s| s.connected);
    tx.send(BrokerCmd::Raw {
        table: Table::T1,
        bytes: vec![0x22, 0x00],
        confirmed_set: false,
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

#[test]
fn playing_compares_output_name_with_the_device_name() {
    assert!(is_playing(Some("Roy's ULT WEAR"), Some("Roy's ULT WEAR")));
    assert!(is_playing(Some("roy's ult wear"), Some("Roy's ULT WEAR")));
    assert!(!is_playing(
        Some("MacBook Pro Speakers"),
        Some("Roy's ULT WEAR")
    ));
    assert!(!is_playing(None, Some("Roy's ULT WEAR")));
    assert!(!is_playing(Some("Roy's ULT WEAR"), None));
}
