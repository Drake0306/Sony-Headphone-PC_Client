//! Per-unit device records: `devices/<address>.json` with the proven functions.

use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver};
use std::time::{Duration, Instant};

use hairline_lib::broker::{run_broker_logged, BrokerCmd, BrokerContext};
use hairline_lib::device_record::{capability_rows, load, save, CapabilityRow, DeviceRecord};
use mdr_proto::Table;
use mdr_session::DeviceState;
use mdr_transport::{Candidate, FakeDevice};

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("hairline-devrec-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

const UNIT_A: &str = "AC:80:0A:11:22:33";
const UNIT_B: &str = "AC:80:0A:44:55:66";

#[test]
fn device_record_round_trip() {
    let dir = temp_dir("round-trip");
    assert_eq!(load(&dir, UNIT_A), DeviceRecord::default());
    let rec = DeviceRecord {
        proven: vec![(Table::T1, 0x50), (Table::T2, 0x46)],
        rfcomm_channel: None,
    };
    save(&dir, UNIT_A, &rec).unwrap();
    assert_eq!(load(&dir, UNIT_A), rec);
    // The address is normalised, so another spelling finds the same file.
    assert_eq!(load(&dir, "ac-80-0a-11-22-33"), rec);
    let file = dir.join("devices").join("ac800a112233.json");
    let text = std::fs::read_to_string(&file).unwrap();
    assert!(text.contains("\"table\": \"T1\""), "{text}");
    assert!(text.contains("\"function\": 80"), "{text}");
    assert!(!dir.join("devices").join("ac800a112233.json.tmp").exists());
    std::fs::write(&file, "{ not json").unwrap();
    assert_eq!(load(&dir, UNIT_A), DeviceRecord::default());
    std::fs::remove_dir_all(&dir).unwrap();
}

fn monotonic_ms() -> u64 {
    static START: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_millis() as u64
}

fn wait_for(rx: &Receiver<DeviceState>, pred: impl Fn(&DeviceState) -> bool) -> DeviceState {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        let s = rx.recv_timeout(left).expect("state before the deadline");
        if pred(&s) {
            return s;
        }
    }
}

/// Connects the fake as `address` with records in `dir`; returns the state once connected.
fn connect_as(dir: &std::path::Path, address: &str) -> DeviceState {
    let candidate = Candidate {
        name: "ULT WEAR".into(),
        address: address.into(),
        connected_audio: true,
    };
    let dir = dir.to_owned();
    let (tx, rx) = channel();
    let (etx, erx) = channel();
    let handle = std::thread::spawn(move || {
        run_broker_logged(
            Box::new(FakeDevice::ult_wear()),
            rx,
            |s| etx.send(s.clone()).unwrap(),
            monotonic_ms,
            BrokerContext {
                device: Some(&candidate),
                record_dir: Some(&dir),
                ..BrokerContext::default()
            },
        )
    });
    let state = wait_for(&erx, |s| s.connected);
    tx.send(BrokerCmd::Disconnect).unwrap();
    assert!(handle.join().unwrap().is_ok());
    state
}

#[test]
fn proven_list_is_per_address() {
    let dir = temp_dir("per-address");
    let rec = DeviceRecord {
        proven: vec![(Table::T1, 0x50)],
        rfcomm_channel: Some(18),
    };
    save(&dir, UNIT_A, &rec).unwrap();
    assert_eq!(load(&dir, UNIT_B), DeviceRecord::default());
    assert_eq!(load(&dir, UNIT_A), rec);

    // Loaded into the session on connect, for that unit only.
    assert_eq!(connect_as(&dir, UNIT_A).proven, vec![(Table::T1, 0x50)]);
    assert!(connect_as(&dir, UNIT_B).proven.is_empty());
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn recorded_channel_round_trip() {
    let dir = temp_dir("channel");
    let rec = DeviceRecord {
        proven: vec![(Table::T1, 0x50)],
        rfcomm_channel: Some(18),
    };
    save(&dir, UNIT_A, &rec).unwrap();
    assert_eq!(load(&dir, UNIT_A), rec);
    let file = dir.join("devices").join("ac800a112233.json");
    let text = std::fs::read_to_string(&file).unwrap();
    assert!(text.contains("\"rfcomm_channel\": 18"), "{text}");

    // A record written before the field existed still loads, without a channel.
    std::fs::write(
        &file,
        r#"{ "proven": [{ "table": "T1", "function": 80 }] }"#,
    )
    .unwrap();
    assert_eq!(
        load(&dir, UNIT_A),
        DeviceRecord {
            proven: vec![(Table::T1, 0x50)],
            rfcomm_channel: None,
        }
    );
    // No channel is not written at all.
    save(&dir, UNIT_B, &DeviceRecord::default()).unwrap();
    let text = std::fs::read_to_string(dir.join("devices").join("ac800a445566.json")).unwrap();
    assert!(!text.contains("rfcomm_channel"), "{text}");
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn connect_records_the_transport_channel() {
    let dir = temp_dir("records-channel");
    // An existing proven list survives the channel being added.
    let rec = DeviceRecord {
        proven: vec![(Table::T1, 0x50)],
        rfcomm_channel: None,
    };
    save(&dir, UNIT_A, &rec).unwrap();
    // The fake reports channel 18.
    connect_as(&dir, UNIT_A);
    assert_eq!(
        load(&dir, UNIT_A),
        DeviceRecord {
            proven: vec![(Table::T1, 0x50)],
            rfcomm_channel: Some(18),
        }
    );
    // A unit without a record gets one holding just the channel.
    connect_as(&dir, UNIT_B);
    assert_eq!(load(&dir, UNIT_B).rfcomm_channel, Some(18));
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn capability_rows_merge_declared_and_proven() {
    let state = DeviceState {
        capabilities_t1: vec![0x53, 0x12],
        capabilities_t2: vec![0x42],
        proven: vec![(Table::T1, 0x12), (Table::T1, 0x50)],
        ..DeviceState::default()
    };
    let rows = capability_rows(&state);
    let row = |table, id: u8, declared, proven| CapabilityRow {
        table,
        id,
        name: mdr_proto::function::name(table, id).map(str::to_owned),
        declared,
        proven,
    };
    assert_eq!(
        rows,
        vec![
            row(Table::T1, 0x12, true, true),
            row(Table::T1, 0x50, false, true),
            row(Table::T1, 0x53, true, false),
            row(Table::T2, 0x42, true, false),
        ]
    );
}
