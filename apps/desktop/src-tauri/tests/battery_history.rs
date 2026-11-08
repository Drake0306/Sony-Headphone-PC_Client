//! Battery history (`battery.jsonl`) and the time-left estimate.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{channel, Receiver};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use mdr_frame::FrameType;
use mdr_session::DeviceState;
use mdr_transport::FakeDevice;
use shdc_lib::battery_history::{
    append, clear, estimate, load, push, BatteryEstimate, BatteryHost, Sample, CAP,
};
use shdc_lib::broker::{run_broker_logged, BrokerCmd, BrokerContext};

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("shdc-battery-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

fn s(t: u64, level: u8, charging: bool) -> Sample {
    Sample { t, level, charging }
}

/// A steady drain: `from` to `to` over `minutes`, one sample per level.
fn drain(start: u64, from: u8, to: u8, minutes: u64) -> Vec<Sample> {
    let steps = u64::from(from - to);
    (0..=steps)
        .map(|i| s(start + i * minutes * 60 / steps, from - i as u8, false))
        .collect()
}

#[test]
fn time_left_from_a_steady_drain() {
    let samples = drain(1_000_000, 100, 90, 60);
    let now = samples.last().unwrap().t;
    let est = estimate(&samples, now);
    let left = est.minutes_left.expect("an estimate");
    assert!((530..=550).contains(&left), "{left}");
    assert_eq!(left % 10, 0);
    assert_eq!(est.samples, 11);
    assert_eq!(est.since, Some(1_000_000));
}

#[test]
fn time_left_blank_cases() {
    let base = drain(1_000_000, 100, 90, 60);
    let end = base.last().unwrap().t;

    // Charging now.
    let mut charging = base.clone();
    charging.push(s(end + 60, 90, true));
    assert_eq!(estimate(&charging, end + 60).minutes_left, None);

    // A gap over 2 h cuts the segment to the samples after it.
    let mut gap = base.clone();
    gap.push(s(end + 121 * 60, 89, false));
    gap.push(s(end + 130 * 60, 88, false));
    assert_eq!(estimate(&gap, end + 130 * 60).minutes_left, None);

    // Less than 30 min of data.
    let short = drain(1_000_000, 100, 90, 29);
    assert_eq!(estimate(&short, short.last().unwrap().t).minutes_left, None);

    // Less than a 2 % drop.
    let flat = vec![s(1_000_000, 80, false), s(1_000_000 + 3600, 79, false)];
    assert_eq!(estimate(&flat, 1_000_000 + 3600).minutes_left, None);

    // A rising level (non-negative slope).
    let rising = vec![
        s(1_000_000, 50, false),
        s(1_000_000 + 1800, 60, false),
        s(1_000_000 + 3600, 70, false),
    ];
    assert_eq!(estimate(&rising, 1_000_000 + 3600).minutes_left, None);

    // Charging in the middle: only the samples after it count.
    let mut after_charge = vec![s(999_000, 40, true)];
    after_charge.extend(drain(1_000_000, 100, 98, 20));
    assert_eq!(
        estimate(&after_charge, after_charge.last().unwrap().t).minutes_left,
        None
    );

    // Nothing recorded for over 2 h.
    assert_eq!(estimate(&base, end + 121 * 60).minutes_left, None);

    // No samples at all.
    assert_eq!(
        estimate(&[], 0),
        BatteryEstimate {
            minutes_left: None,
            samples: 0,
            since: None
        }
    );
}

#[test]
fn a_plateau_inside_a_drain_keeps_the_estimate() {
    // 100 -> 95 over 30 min, 45 min flat at 95, then 95 -> 90 over 30 min.
    let mut samples = drain(1_000_000, 100, 95, 30);
    let flat_end = samples.last().unwrap().t + 45 * 60;
    samples.extend(drain(flat_end, 94, 90, 24));
    let now = samples.last().unwrap().t;
    let est = estimate(&samples, now);
    assert!(est.minutes_left.is_some(), "{est:?}");
    // Flat for 45 min since the last sample: still within the gap rule.
    assert!(estimate(&samples, now + 45 * 60).minutes_left.is_some());
}

#[test]
fn appending_after_a_partial_line_starts_a_new_one() {
    let dir = temp_dir("partial");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("battery.jsonl"),
        "{\"t\":10,\"level\":80,\"charging\":false}\n{\"t\":20,\"lev",
    )
    .unwrap();
    assert!(append(&dir, s(30, 79, false)).unwrap());
    assert_eq!(load(&dir), [s(10, 80, false), s(30, 79, false)]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn samples_append_only_on_change() {
    let dir = temp_dir("append");
    assert!(append(&dir, s(10, 80, false)).unwrap());
    assert!(!append(&dir, s(20, 80, false)).unwrap());
    assert!(append(&dir, s(30, 79, false)).unwrap());
    assert!(append(&dir, s(40, 79, true)).unwrap());
    assert!(!append(&dir, s(50, 79, true)).unwrap());
    // The connect sample is written even when nothing changed.
    push(&dir, s(60, 79, true)).unwrap();
    let got = load(&dir);
    assert_eq!(
        got,
        [
            s(10, 80, false),
            s(30, 79, false),
            s(40, 79, true),
            s(60, 79, true)
        ]
    );
    let text = std::fs::read_to_string(dir.join("battery.jsonl")).unwrap();
    assert_eq!(
        text.lines().next(),
        Some(r#"{"t":10,"level":80,"charging":false}"#)
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_file_keeps_the_newest_samples() {
    let dir = temp_dir("cap");
    std::fs::create_dir_all(&dir).unwrap();
    let lines: String = (0..CAP as u64)
        .map(|t| format!("{{\"t\":{t},\"level\":{},\"charging\":false}}\n", t % 2))
        .collect();
    std::fs::write(dir.join("battery.jsonl"), lines).unwrap();
    assert!(append(&dir, s(CAP as u64, 50, false)).unwrap());
    let got = load(&dir);
    assert_eq!(got.len(), CAP);
    assert_eq!(got.first().unwrap().t, 1);
    assert_eq!(got.last().unwrap(), &s(CAP as u64, 50, false));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn clear_removes_the_file() {
    let dir = temp_dir("clear");
    push(&dir, s(10, 80, false)).unwrap();
    assert!(dir.join("battery.jsonl").exists());
    clear(&dir).unwrap();
    assert!(!dir.join("battery.jsonl").exists());
    assert!(load(&dir).is_empty());
    // Clearing nothing is fine.
    clear(&dir).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
}

fn monotonic_ms() -> u64 {
    static START: OnceLock<Instant> = OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_millis() as u64
}

fn next_estimate(rx: &Receiver<BatteryEstimate>) -> BatteryEstimate {
    rx.recv_timeout(Duration::from_secs(5))
        .expect("a battery event")
}

#[test]
fn battery_notify_appends_and_emits() {
    let dir = temp_dir("broker");
    let mut fake = FakeDevice::ult_wear();
    // A read nobody else sends, answered with a battery NTFY (64 %).
    fake.answer(
        FrameType::Command1,
        &[0x22, 0x01],
        &[0x25, 0x00, 0x40, 0x00],
    );
    let (tx, rx) = channel();
    let (btx, brx) = channel();
    let (stx, srx) = channel::<DeviceState>();
    let wall = Arc::new(Mutex::new(5_000u64));
    let thread_dir = dir.clone();
    let thread_wall = Arc::clone(&wall);
    let handle = std::thread::spawn(move || {
        let mut host = BatteryHost::new(
            thread_dir,
            Box::new(move || *thread_wall.lock().unwrap()),
            Box::new(move |e: &BatteryEstimate| btx.send(e.clone()).unwrap()),
        );
        run_broker_logged(
            Box::new(fake),
            rx,
            |st: &DeviceState| {
                let _ = stx.send(st.clone());
            },
            monotonic_ms,
            BrokerContext {
                battery: Some(&mut host),
                ..BrokerContext::default()
            },
        )
    });
    // The connect sample: the handshake's battery reply (67 %).
    let first = next_estimate(&brx);
    assert_eq!(first.samples, 1);
    assert_eq!(first.since, Some(5_000));
    assert_eq!(load(&dir), [s(5_000, 67, false)]);

    *wall.lock().unwrap() = 5_060;
    tx.send(BrokerCmd::Raw {
        table: mdr_proto::Table::T1,
        bytes: vec![0x22, 0x01],
        confirmed_set: false,
    })
    .unwrap();
    let second = next_estimate(&brx);
    assert_eq!(second.samples, 2);
    assert_eq!(second.minutes_left, None);
    assert_eq!(load(&dir), [s(5_000, 67, false), s(5_060, 64, false)]);

    tx.send(BrokerCmd::Disconnect).unwrap();
    assert!(handle.join().unwrap().is_ok());
    drop(srx);
    let _ = std::fs::remove_dir_all(&dir);
}

/// Runs the broker on the fake with a battery host; the broker's clock is
/// `monotonic_ms` plus `skew_ms`, the wall clock is `wall` (unix seconds).
fn spawn_with_battery(
    dir: PathBuf,
    wall: Arc<AtomicU64>,
    skew_ms: Arc<AtomicU64>,
) -> (
    std::sync::mpsc::Sender<BrokerCmd>,
    Receiver<BatteryEstimate>,
    std::thread::JoinHandle<std::io::Result<()>>,
) {
    let (tx, rx) = channel();
    let (btx, brx) = channel();
    let handle = std::thread::spawn(move || {
        let mut host = BatteryHost::new(
            dir,
            Box::new(move || wall.load(Ordering::SeqCst)),
            Box::new(move |e: &BatteryEstimate| {
                let _ = btx.send(e.clone());
            }),
        );
        run_broker_logged(
            Box::new(FakeDevice::ult_wear()),
            rx,
            |_: &DeviceState| {},
            move || monotonic_ms() + skew_ms.load(Ordering::SeqCst),
            BrokerContext {
                battery: Some(&mut host),
                ..BrokerContext::default()
            },
        )
    });
    (tx, brx, handle)
}

#[test]
fn the_estimate_is_re_emitted_and_goes_blank_when_stale() {
    let dir = temp_dir("stale");
    const W: u64 = 2_000_000;
    // 77 -> 68 over the hour before connecting; the handshake reports 67 %.
    for sample in drain(W - 3600, 77, 68, 59) {
        push(&dir, sample).unwrap();
    }
    let wall = Arc::new(AtomicU64::new(W));
    let skew = Arc::new(AtomicU64::new(0));
    let (tx, brx, handle) = spawn_with_battery(dir.clone(), Arc::clone(&wall), Arc::clone(&skew));
    // The first event comes from the write; the re-emit timer is armed on the
    // broker clock read before it, so skewing the clock only after this
    // receive makes the next pass due.
    let first = next_estimate(&brx);
    assert_eq!(first.samples, 11);
    assert!(first.minutes_left.is_some(), "{first:?}");

    // Flat at 67 % for over 2 h: the next timed re-emit is blank.
    wall.store(W + 121 * 60, Ordering::SeqCst);
    skew.store(61_000, Ordering::SeqCst);
    let later = next_estimate(&brx);
    assert_eq!(later.minutes_left, None);
    assert_eq!(later.samples, 11);

    tx.send(BrokerCmd::Disconnect).unwrap();
    assert!(handle.join().unwrap().is_ok());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn clearing_takes_a_fresh_sample_while_connected() {
    let dir = temp_dir("cleared");
    let wall = Arc::new(AtomicU64::new(3_000));
    let skew = Arc::new(AtomicU64::new(0));
    let (tx, brx, handle) = spawn_with_battery(dir.clone(), Arc::clone(&wall), skew);
    assert_eq!(next_estimate(&brx).samples, 1);

    clear(&dir).unwrap();
    wall.store(3_100, Ordering::SeqCst);
    tx.send(BrokerCmd::BatteryHistoryCleared).unwrap();
    let after = next_estimate(&brx);
    assert_eq!(after.samples, 1);
    assert_eq!(after.since, Some(3_100));
    assert_eq!(load(&dir), [s(3_100, 67, false)]);

    tx.send(BrokerCmd::Disconnect).unwrap();
    assert!(handle.join().unwrap().is_ok());
    let _ = std::fs::remove_dir_all(&dir);
}
