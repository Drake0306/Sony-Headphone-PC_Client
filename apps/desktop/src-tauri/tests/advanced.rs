//! The Advanced backend: raw commands, the frame stream and frame recording.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use hairline_lib::broker::{raw_command, run_broker_logged, BrokerCmd, BrokerContext};
use hairline_lib::frame_ring::{FrameDir, FrameEntry, FrameTap, Recorder};
use mdr_proto::Table;
use mdr_session::DeviceState;
use mdr_transport::FakeDevice;

const VOLUME_SET: [u8; 3] = [0xA8, 0x20, 0x0F];
const BATTERY_GET: [u8; 2] = [0x22, 0x00];

#[test]
fn raw_set_needs_confirmation() {
    let refused = Err("type SEND to send a SET".to_string());
    assert_eq!(raw_command("t1", "A8 20 0F", ""), refused);
    assert_eq!(raw_command("t1", "A8 20 0F", "send"), refused);
    assert_eq!(raw_command("t2", "48 20 01", "yes"), refused);
    // Deny-listed payloads are refused even when confirmed.
    for denied in ["24 03 01", "F8 09 01", "C4 00", "98 00"] {
        let result = raw_command("t1", denied, "SEND");
        assert!(
            result.as_ref().is_err_and(|e| e.starts_with("never sent")),
            "{denied}: {result:?}"
        );
    }
    // Opcodes missing from the role table count as SETs.
    assert_eq!(raw_command("t1", "0F 00", ""), refused);
    assert_eq!(
        raw_command("t1", "A8 20 0F", "SEND"),
        Ok(BrokerCmd::Raw {
            table: Table::T1,
            bytes: VOLUME_SET.to_vec(),
            confirmed_set: true,
        })
    );

    // The broker refuses an unconfirmed SET that reaches it some other way.
    let rig = Rig::start(None);
    rig.cmd(BrokerCmd::Raw {
        table: Table::T1,
        bytes: VOLUME_SET.to_vec(),
        confirmed_set: false,
    });
    rig.cmd(BrokerCmd::Raw {
        table: Table::T1,
        bytes: BATTERY_GET.to_vec(),
        confirmed_set: false,
    });
    let frames = rig.frames_until(|f| sent(f, &BATTERY_GET));
    assert!(!sent(&frames, &VOLUME_SET), "unconfirmed SET was sent");
    rig.cmd(BrokerCmd::Raw {
        table: Table::T1,
        bytes: VOLUME_SET.to_vec(),
        confirmed_set: true,
    });
    rig.frames_until(|f| sent(f, &VOLUME_SET));
    rig.stop();
}

#[test]
fn raw_get_needs_no_confirmation() {
    assert_eq!(
        raw_command("t1", "22 00", ""),
        Ok(BrokerCmd::Raw {
            table: Table::T1,
            bytes: BATTERY_GET.to_vec(),
            confirmed_set: false,
        })
    );
    assert!(raw_command("t2", "46 20", "").is_ok());
    assert!(raw_command("t1", "zz", "SEND").is_err());
}

#[test]
fn watch_frames_streams_sent_and_received() {
    let rig = Rig::start(None);
    // The handshake happened before watching started; the snapshot still has it.
    let frames = rig.frames_until(|f| {
        f.iter()
            .any(|e| e.dir == FrameDir::Sent && e.table == Some(Table::T1))
            && f.iter()
                .any(|e| e.dir == FrameDir::Received && e.table == Some(Table::T1))
    });
    // The protocol-info GET opens the handshake.
    assert!(sent(&frames, &[0x00, 0x00]), "{frames:?}");
    assert!(frames.len() <= 500);
    // New frames keep coming while watched.
    rig.cmd(BrokerCmd::Raw {
        table: Table::T1,
        bytes: BATTERY_GET.to_vec(),
        confirmed_set: false,
    });
    rig.frames_until(|f| sent(f, &BATTERY_GET));
    // Unwatched: nothing more is emitted.
    rig.cmd(BrokerCmd::WatchFrames(false));
    std::thread::sleep(Duration::from_millis(100));
    while rig.frames.try_recv().is_ok() {}
    rig.cmd(BrokerCmd::Refresh);
    std::thread::sleep(Duration::from_millis(300));
    assert!(
        rig.frames.try_recv().is_err(),
        "frames emitted while unwatched"
    );
    rig.stop();
}

#[test]
fn record_frames_creates_and_closes_a_file() {
    let dir = std::env::temp_dir().join(format!("hairline-record-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let rig = Rig::start(Some(dir.clone()));
    rig.cmd(BrokerCmd::RecordFrames(true));
    let path = rig
        .recording
        .recv_timeout(Duration::from_secs(5))
        .unwrap()
        .expect("a recording path");
    assert!(path.starts_with(&dir), "{path:?}");
    assert!(path.exists());
    rig.cmd(BrokerCmd::Raw {
        table: Table::T1,
        bytes: BATTERY_GET.to_vec(),
        confirmed_set: false,
    });
    rig.frames_until(|f| sent(f, &BATTERY_GET));
    std::thread::sleep(Duration::from_millis(200));
    rig.cmd(BrokerCmd::RecordFrames(false));
    let stopped = rig.recording.recv_timeout(Duration::from_secs(5)).unwrap();
    assert_eq!(stopped, None);
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.lines().any(|l| l.contains(" > ")), "{text}");
    assert!(text.lines().any(|l| l.contains(" < ")), "{text}");
    // Closed: later frames are not written.
    rig.cmd(BrokerCmd::Refresh);
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(std::fs::read_to_string(&path).unwrap(), text);
    rig.stop();
    std::fs::remove_dir_all(&dir).unwrap();
}

/// Whether a sent frame carries `payload`.
fn sent(frames: &[FrameEntry], payload: &[u8]) -> bool {
    frames
        .iter()
        .any(|e| e.dir == FrameDir::Sent && e.bytes.windows(payload.len()).any(|w| w == payload))
}

fn monotonic_ms() -> u64 {
    static START: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_millis() as u64
}

/// A broker on the fake device, watching frames from the start.
struct Rig {
    tx: Sender<BrokerCmd>,
    frames: Receiver<Vec<FrameEntry>>,
    recording: Receiver<Option<PathBuf>>,
    handle: JoinHandle<std::io::Result<()>>,
}

impl Rig {
    fn start(log_dir: Option<PathBuf>) -> Rig {
        let (tx, rx) = channel();
        let (ftx, frames) = channel();
        let (rtx, recording) = channel();
        let (stx, states) = channel();
        let log_dir = log_dir.unwrap_or_else(|| std::env::temp_dir().join("hairline-unused"));
        let handle = std::thread::spawn(move || {
            let mut tap = FrameTap::new(
                log_dir,
                Recorder::default(),
                Box::new(move |f: &[FrameEntry]| {
                    let _ = ftx.send(f.to_vec());
                }),
                Box::new(move |p: Option<&Path>| {
                    let _ = rtx.send(p.map(Path::to_owned));
                }),
            );
            run_broker_logged(
                Box::new(FakeDevice::ult_wear()),
                rx,
                move |s: &DeviceState| {
                    let _ = stx.send(s.connected);
                },
                monotonic_ms,
                BrokerContext {
                    frames: Some(&mut tap),
                    ..BrokerContext::default()
                },
            )
        });
        let deadline = Instant::now() + Duration::from_secs(5);
        while !states
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .expect("connected before the deadline")
        {}
        tx.send(BrokerCmd::WatchFrames(true)).unwrap();
        Rig {
            tx,
            frames,
            recording,
            handle,
        }
    }

    fn cmd(&self, cmd: BrokerCmd) {
        self.tx.send(cmd).unwrap();
    }

    /// The first emitted snapshot that satisfies `pred`.
    fn frames_until(&self, pred: impl Fn(&[FrameEntry]) -> bool) -> Vec<FrameEntry> {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            let f = self
                .frames
                .recv_timeout(left)
                .expect("frames before the deadline");
            if pred(&f) {
                return f;
            }
        }
    }

    fn stop(self) {
        self.tx.send(BrokerCmd::Disconnect).unwrap();
        assert!(self.handle.join().unwrap().is_ok());
    }
}
