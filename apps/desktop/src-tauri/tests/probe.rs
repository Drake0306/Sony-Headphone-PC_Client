//! The opcode probe on the fake device: one GET at a time, never a SET or a
//! denied byte, undeclared answers proven, stops on Stop and on disconnect.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use mdr_frame::FrameType;
use mdr_proto::function::{opcode_role, Role};
use mdr_proto::probe::{probe_points, ProbePoint, GET_CATALOGUE};
use mdr_proto::Table;
use mdr_session::DeviceState;
use mdr_transport::{Candidate, FakeDevice, ReceivedLog};
use shdc_lib::broker::{run_broker_logged, BrokerCmd, BrokerContext};
use shdc_lib::device_record;
use shdc_lib::probe::{probe_allowed, ProbeEvent, ProbeHost, ProbeRunner, ProbeState};
use shdc_lib::settings::AppSettings;

const UNIT: &str = "AC:80:0A:77:88:99";

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("shdc-probe-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

fn monotonic_ms() -> u64 {
    static START: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_millis() as u64
}

/// Catalogue points for the given (table, opcode) pairs only.
fn points_for(ops: &[(Table, u8)]) -> Vec<ProbePoint> {
    probe_points(GET_CATALOGUE)
        .into_iter()
        .filter(|p| ops.contains(&(p.table, p.opcode)))
        .collect()
}

/// The fake answering two undeclared GETs (T2 `32 02`, T1 `F6 0F`).
fn scripted_fake() -> FakeDevice {
    let mut fake = FakeDevice::ult_wear();
    fake.answer(
        FrameType::Command2,
        &[0x32, 0x02],
        &[0x33, 0x02, 0x00, 0x01],
    );
    fake.answer(FrameType::Command1, &[0xF6, 0x0F], &[0xF7, 0x0F, 0x01]);
    fake
}

struct Rig {
    tx: Sender<BrokerCmd>,
    states: Receiver<DeviceState>,
    probe: Receiver<ProbeEvent>,
    log: ReceivedLog,
    handle: JoinHandle<std::io::Result<()>>,
}

impl Rig {
    fn start(fake: FakeDevice, runner: ProbeRunner, dir: &Path) -> Rig {
        let log = fake.received_log();
        let (tx, rx) = channel();
        let (stx, states) = channel();
        let (ptx, probe) = channel();
        let dir = dir.to_owned();
        let handle = std::thread::spawn(move || {
            let candidate = Candidate {
                name: "ULT WEAR".into(),
                address: UNIT.into(),
                connected_audio: true,
            };
            let mut host = ProbeHost::new(
                runner,
                dir.join("captures"),
                Box::new(move |e: &ProbeEvent| {
                    let _ = ptx.send(e.clone());
                }),
            );
            run_broker_logged(
                Box::new(fake),
                rx,
                move |s: &DeviceState| {
                    let _ = stx.send(s.clone());
                },
                monotonic_ms,
                BrokerContext {
                    device: Some(&candidate),
                    record_dir: Some(&dir),
                    probe: Some(&mut host),
                    ..BrokerContext::default()
                },
            )
        });
        let rig = Rig {
            tx,
            states,
            probe,
            log,
            handle,
        };
        rig.state_until(|s| s.connected);
        rig
    }

    fn state_until(&self, pred: impl Fn(&DeviceState) -> bool) -> DeviceState {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            let s = self
                .states
                .recv_timeout(left)
                .expect("state before the deadline");
            if pred(&s) {
                return s;
            }
        }
    }

    /// Probe events up to and including the first that satisfies `pred`.
    fn probe_until(&self, pred: impl Fn(&ProbeEvent) -> bool) -> Vec<ProbeEvent> {
        let deadline = Instant::now() + Duration::from_secs(20);
        let mut seen = Vec::new();
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            let e = self
                .probe
                .recv_timeout(left)
                .expect("probe event before the deadline");
            let done = pred(&e);
            seen.push(e);
            if done {
                return seen;
            }
        }
    }

    /// How many frames carrying exactly `payload` the fake received.
    fn count_sent(&self, payload: &[u8]) -> usize {
        self.log
            .lock()
            .unwrap()
            .iter()
            .filter(|(_, p)| p == payload)
            .count()
    }

    /// Polls `cond` until it holds (10 s at most).
    fn wait_until(&self, cond: impl Fn() -> bool) {
        let deadline = Instant::now() + Duration::from_secs(10);
        while !cond() {
            assert!(Instant::now() < deadline, "condition not met in time");
            std::thread::sleep(Duration::from_millis(2));
        }
    }

    fn sent_count(&self) -> usize {
        self.log.lock().unwrap().len()
    }

    fn stop(self) -> ReceivedLog {
        let _ = self.tx.send(BrokerCmd::Disconnect);
        let _ = self.handle.join().unwrap();
        self.log
    }
}

/// No frame the fake received was a SET, an unknown opcode or deny-listed.
fn assert_get_only(log: &ReceivedLog) {
    for (frame_type, payload) in log.lock().unwrap().iter() {
        let table = match frame_type {
            FrameType::Command1 => Table::T1,
            FrameType::Command2 => Table::T2,
            _ => continue,
        };
        assert_eq!(
            opcode_role(table, payload[0]),
            Role::Get,
            "{table:?} {payload:02X?} is not a GET"
        );
        assert_eq!(mdr_proto::denied(table, payload), None, "{payload:02X?}");
    }
}

#[test]
fn probe_proves_exactly_the_undeclared_answers() {
    let dir = temp_dir("proves");
    let mut points = points_for(&[
        (Table::T2, 0x32),
        (Table::T2, 0x46),
        (Table::T1, 0x22),
        (Table::T1, 0xF6),
    ]);
    // A wrongly edited list: these must never reach the device.
    for (opcode, inquired) in [(0x24, 0x00), (0xF8, 0x09), (0x98, 0x00)] {
        points.push(ProbePoint {
            table: Table::T1,
            opcode,
            inquired: Some(inquired),
            name: "BAD",
        });
    }
    let sendable = points.len() - 3;
    let rig = Rig::start(
        scripted_fake(),
        ProbeRunner::with_timing(60, 5).with_points(points),
        &dir,
    );
    let before = rig.sent_count();
    rig.tx.send(BrokerCmd::ProbeStart).unwrap();
    let events = rig.probe_until(|e| e.state == ProbeState::Done);

    assert_eq!(events[0].state, ProbeState::Running);
    assert_eq!(events[0].done, 0);
    assert!(events[..events.len() - 1]
        .iter()
        .all(|e| e.state == ProbeState::Running));
    let done = events.last().unwrap();
    assert_eq!(done.total, sendable);
    assert_eq!(done.done, sendable);
    assert_eq!(done.results.len(), sendable);

    let proven: Vec<_> = done
        .results
        .iter()
        .filter(|r| r.proven)
        .map(|r| (r.table, r.opcode, r.inquired, r.function))
        .collect();
    assert_eq!(
        proven,
        vec![
            (Table::T1, 0xF6, Some(0x0F), Some(0xFF)),
            (Table::T2, 0x32, Some(0x02), Some(0x32)),
        ]
    );
    let find = |table, opcode, inquired| {
        done.results
            .iter()
            .find(|r| r.table == table && r.opcode == opcode && r.inquired == Some(inquired))
            .unwrap()
    };
    let t2_32 = find(Table::T2, 0x32, 0x02);
    assert_eq!(t2_32.reply.as_deref(), Some(&[0x33, 0x02, 0x00, 0x01][..]));
    // Answered but declared: not proven.
    let battery = find(Table::T1, 0x22, 0x00);
    assert!(battery.declared && !battery.proven && battery.reply.is_some());
    assert!(find(Table::T2, 0x46, 0x20).declared);
    // Silent.
    let silent = find(Table::T1, 0x22, 0x0E);
    assert_eq!(silent.reply, None);
    assert!(!silent.proven);

    // Marked in the session, saved to the unit's record, written to a file.
    let state = rig.state_until(|s| s.proven.len() == 2);
    assert!(state.proven.contains(&(Table::T2, 0x32)));
    assert!(state.proven.contains(&(Table::T1, 0xFF)));
    let record = device_record::load(&dir, UNIT);
    assert_eq!(record.proven, vec![(Table::T1, 0xFF), (Table::T2, 0x32)]);
    let file = PathBuf::from(done.file.as_deref().expect("results file"));
    assert!(file.starts_with(dir.join("captures")), "{file:?}");
    let name = file.file_name().unwrap().to_string_lossy().into_owned();
    assert!(
        name.starts_with("probe-") && name.ends_with(".json"),
        "{name}"
    );
    let json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
    assert_eq!(json["results"].as_array().unwrap().len(), sendable);

    // Exactly one request per sendable point, and every one a GET.
    assert_eq!(rig.sent_count() - before, sendable);
    let log = rig.stop();
    assert_get_only(&log);
    let sent: Vec<_> = log.lock().unwrap().iter().map(|(_, p)| p.clone()).collect();
    for bad in [[0x24, 0x00], [0xF8, 0x09], [0x98, 0x00]] {
        assert!(!sent.iter().any(|p| p.starts_with(&bad)), "{bad:02X?} sent");
    }
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_proven_function_does_not_unlock_its_typed_set() {
    let dir = temp_dir("proven-set");
    // Touch panel (D2) answered but undeclared: the probe proves it.
    let mut fake = FakeDevice::ult_wear();
    fake.without_capability(0xD2);
    let points: Vec<_> = points_for(&[(Table::T1, 0xD6)])
        .into_iter()
        .filter(|p| p.inquired == Some(0xD2))
        .collect();
    assert_eq!(points.len(), 1);
    let rig = Rig::start(
        fake,
        ProbeRunner::with_timing(60, 5).with_points(points),
        &dir,
    );
    rig.tx.send(BrokerCmd::ProbeStart).unwrap();
    rig.probe_until(|e| e.state == ProbeState::Done);
    rig.state_until(|s| s.proven.contains(&(Table::T1, 0xD2)));

    // The handshake reads volume too, so count the GETs sent after the SET.
    let before = rig.count_sent(&[0xA6, 0x20]);
    rig.tx.send(BrokerCmd::SetTouchPanel(false)).unwrap();
    // A GET sent after it: once it is on the wire, an accepted SET would be too.
    rig.tx.send(user_get()).unwrap();
    rig.wait_until(|| rig.count_sent(&[0xA6, 0x20]) > before);
    let log = rig.stop();
    assert!(
        !log.lock().unwrap().iter().any(|(_, p)| p[0] == 0xD8),
        "a typed SET on a proven function was written"
    );
    assert_get_only(&log);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn probe_stop_sends_nothing_more() {
    let dir = temp_dir("stop");
    let points = points_for(&[(Table::T1, 0xF0), (Table::T1, 0xF2)]);
    let total = points.len();
    let rig = Rig::start(
        FakeDevice::ult_wear(),
        ProbeRunner::with_timing(150, 5).with_points(points),
        &dir,
    );
    rig.tx.send(BrokerCmd::ProbeStart).unwrap();
    rig.probe_until(|e| e.state == ProbeState::Running && e.done >= 2);
    rig.tx.send(BrokerCmd::ProbeStop).unwrap();
    let done = rig
        .probe_until(|e| e.state == ProbeState::Done)
        .pop()
        .unwrap();
    assert!(done.done < total, "{} of {total}", done.done);
    assert_eq!(done.results.len(), done.done);
    assert!(done.file.is_some());
    let after_stop = rig.sent_count();
    std::thread::sleep(Duration::from_millis(600));
    assert_eq!(rig.sent_count(), after_stop, "sent after Stop");
    // A second Stop is harmless; a fresh start resets the progress.
    rig.tx.send(BrokerCmd::ProbeStop).unwrap();
    let log = rig.stop();
    assert_get_only(&log);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn probe_ends_when_the_link_drops() {
    let dir = temp_dir("drop");
    // How many frames the fake receives before it is connected.
    let rig = Rig::start(FakeDevice::ult_wear(), ProbeRunner::new(), &dir);
    let handshake = rig.sent_count();
    rig.stop();

    let mut fake = FakeDevice::ult_wear();
    // The handshake (plus ACKs, counted as frames too) and a few probe requests.
    fake.drop_link_after(handshake * 2 + 6);
    let points = points_for(&[(Table::T1, 0xF0), (Table::T1, 0xF2)]);
    let total = points.len();
    let rig = Rig::start(
        fake,
        ProbeRunner::with_timing(40, 5).with_points(points),
        &dir,
    );
    rig.tx.send(BrokerCmd::ProbeStart).unwrap();
    let done = rig
        .probe_until(|e| e.state == ProbeState::Done)
        .pop()
        .unwrap();
    assert!(done.done < total, "{} of {total}", done.done);
    assert!(rig.handle.join().unwrap().is_err(), "the link was lost");
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn probe_refused_until_connected_enabled_and_confirmed() {
    let off = AppSettings::default();
    let on = AppSettings {
        show_advanced: true,
        ..AppSettings::default()
    };
    let connected = DeviceState {
        connected: true,
        ..DeviceState::default()
    };
    assert_eq!(
        probe_allowed(&off, &connected, "RUN"),
        Err("enable the Advanced panel in Settings first".to_string())
    );
    assert_eq!(
        probe_allowed(&on, &DeviceState::default(), "RUN"),
        Err("not connected".to_string())
    );
    for wrong in ["", "run", "RUN "] {
        assert_eq!(
            probe_allowed(&on, &connected, wrong),
            Err("type RUN to start the probe".to_string())
        );
    }
    assert_eq!(probe_allowed(&on, &connected, "RUN"), Ok(()));
}

#[test]
fn probe_start_while_connecting_does_nothing() {
    // ProbeStart before the handshake finishes is ignored (not queued).
    let dir = temp_dir("early");
    let (tx, rx) = channel();
    let (ptx, prx) = channel();
    tx.send(BrokerCmd::ProbeStart).unwrap();
    let fake = FakeDevice::ult_wear();
    let log = fake.received_log();
    let d = dir.clone();
    let handle = std::thread::spawn(move || {
        let mut host = ProbeHost::new(
            ProbeRunner::with_timing(20, 0),
            d.join("captures"),
            Box::new(move |e: &ProbeEvent| {
                let _ = ptx.send(e.clone());
            }),
        );
        run_broker_logged(
            Box::new(fake),
            rx,
            |_: &DeviceState| {},
            monotonic_ms,
            BrokerContext {
                probe: Some(&mut host),
                ..BrokerContext::default()
            },
        )
    });
    std::thread::sleep(Duration::from_millis(400));
    tx.send(BrokerCmd::Disconnect).unwrap();
    handle.join().unwrap().unwrap();
    assert!(prx.try_iter().all(|e| e.state != ProbeState::Running));
    // Only the handshake went out: nothing from the probe catalogue's extra reads.
    let sent: Vec<_> = log.lock().unwrap().iter().map(|(_, p)| p.clone()).collect();
    assert!(!sent.iter().any(|p| p == &[0x02, 0x00]), "{sent:02X?}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// A user GET whose ACK the fake holds back, so the probe's first request
/// waits in the ARQ queue behind it.
fn backlogged_fake() -> FakeDevice {
    let mut fake = FakeDevice::ult_wear();
    fake.answer(
        FrameType::Command2,
        &[0x32, 0x02],
        &[0x33, 0x02, 0x00, 0x01],
    );
    fake.delay_ack(&[0xA6, 0x20], Duration::from_millis(400));
    fake
}

fn user_get() -> BrokerCmd {
    BrokerCmd::Raw {
        table: Table::T1,
        bytes: vec![0xA6, 0x20],
        confirmed_set: false,
    }
}

fn t2_32_02() -> Vec<ProbePoint> {
    points_for(&[(Table::T2, 0x32)])
        .into_iter()
        .filter(|p| p.inquired == Some(0x02))
        .collect()
}

#[test]
fn reply_wait_starts_when_the_request_is_on_the_wire() {
    let dir = temp_dir("backlog");
    // The wait (100 ms) is shorter than the backlog (400 ms).
    let rig = Rig::start(
        backlogged_fake(),
        ProbeRunner::with_timing(100, 5).with_points(t2_32_02()),
        &dir,
    );
    rig.tx.send(user_get()).unwrap();
    rig.tx.send(BrokerCmd::ProbeStart).unwrap();
    let done = rig
        .probe_until(|e| e.state == ProbeState::Done)
        .pop()
        .unwrap();
    assert_eq!(done.results.len(), 1);
    let r = &done.results[0];
    assert_eq!(
        r.reply.as_deref(),
        Some(&[0x33, 0x02, 0x00, 0x01][..]),
        "{r:?}"
    );
    assert!(r.proven);
    rig.stop();
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn probe_stop_recalls_a_get_still_queued() {
    let dir = temp_dir("recall");
    let rig = Rig::start(
        backlogged_fake(),
        ProbeRunner::with_timing(100, 5).with_points(t2_32_02()),
        &dir,
    );
    // The handshake reads volume too; count from here.
    let before = rig.count_sent(&[0xA6, 0x20]);
    rig.tx.send(user_get()).unwrap();
    rig.tx.send(BrokerCmd::ProbeStart).unwrap();
    // The probe queues its GET as it starts, behind the user GET, whose ACK
    // the fake holds back; Stop once both have happened.
    rig.probe_until(|e| e.state == ProbeState::Running);
    rig.wait_until(|| rig.count_sent(&[0xA6, 0x20]) > before);
    rig.tx.send(BrokerCmd::ProbeStop).unwrap();
    let done = rig
        .probe_until(|e| e.state == ProbeState::Done)
        .pop()
        .unwrap();
    assert!(done.results.is_empty(), "{:?}", done.results);
    std::thread::sleep(Duration::from_millis(700));
    let log = rig.stop();
    let sent: Vec<_> = log
        .lock()
        .unwrap()
        .iter()
        .map(|(t, p)| (*t, p.clone()))
        .collect();
    assert!(
        sent.iter().any(|(_, p)| p == &[0xA6, 0x20]),
        "the user GET went out"
    );
    assert!(
        !sent
            .iter()
            .any(|(t, p)| *t == FrameType::Command2 && p == &[0x32, 0x02]),
        "the recalled probe GET was sent"
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn idle_event_carries_the_size_and_timing() {
    let runner = ProbeRunner::new();
    let e = runner.event(None);
    assert_eq!(e.state, ProbeState::Idle);
    assert_eq!(e.total, probe_points(GET_CATALOGUE).len());
    assert_eq!((e.reply_ms, e.gap_ms), (1500, 150));
}
