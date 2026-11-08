//! The Advanced backend: raw commands, the frame stream and frame recording.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use mdr_proto::Table;
use mdr_session::DeviceState;
use mdr_transport::FakeDevice;
use shdc_lib::broker::{raw_command, run_broker_logged, BrokerCmd, BrokerContext, RawEvent};
use shdc_lib::frame_ring::{FrameDir, FrameEntry, FrameTap, Recorder};
use shdc_lib::settings::{raw_allowed, AppSettings};

const VOLUME_SET: [u8; 3] = [0xA8, 0x20, 0x0F];
const BATTERY_GET: [u8; 2] = [0x22, 0x00];

#[test]
fn raw_set_needs_confirmation() {
    let refused = Err("type SEND to send a SET".to_string());
    // Every T1 SET in the role table, and T2 voice guidance SET.
    let t1_sets = [
        0x18, 0x24, 0x28, 0x58, 0x68, 0x98, 0xA4, 0xA8, 0xC4, 0xD8, 0xE8, 0xF4, 0xF8, 0xFC,
    ];
    let cases = t1_sets
        .iter()
        .map(|op| ("t1", format!("{op:02X} 01 00")))
        .chain(std::iter::once(("t2", "48 20 01".to_string())));
    for (table, hex) in cases {
        let result = raw_command(table, &hex, "");
        assert!(result.is_err(), "{table} {hex} went through: {result:?}");
        if !result.as_ref().unwrap_err().starts_with("never sent") {
            assert_eq!(result, refused, "{table} {hex}");
            assert!(raw_command(table, &hex, "SEND").is_ok(), "{table} {hex}");
        }
    }
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
fn raw_needs_the_advanced_panel() {
    let mut s = AppSettings::default();
    assert_eq!(
        raw_allowed(&s),
        Err("enable the Advanced panel in Settings first".to_string())
    );
    s.show_advanced = true;
    assert_eq!(raw_allowed(&s), Ok(()));
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
    let dir = std::env::temp_dir().join(format!("shdc-record-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let rig = Rig::start(Some(dir.clone()));
    // Watching (from Rig::start) re-emits the recording state: none yet.
    assert_eq!(
        rig.recording.recv_timeout(Duration::from_secs(5)).unwrap(),
        None
    );
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

#[test]
fn watch_frames_re_emits_the_recording_state() {
    let dir = std::env::temp_dir().join(format!("shdc-rewatch-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let rig = Rig::start(Some(dir.clone()));
    assert_eq!(
        rig.recording.recv_timeout(Duration::from_secs(5)).unwrap(),
        None
    );
    rig.cmd(BrokerCmd::RecordFrames(true));
    let path = rig.recording.recv_timeout(Duration::from_secs(5)).unwrap();
    assert!(path.is_some());
    // A remounted Advanced screen asks to watch again and learns the file.
    rig.cmd(BrokerCmd::WatchFrames(false));
    rig.cmd(BrokerCmd::WatchFrames(true));
    assert_eq!(
        rig.recording.recv_timeout(Duration::from_secs(5)).unwrap(),
        path
    );
    rig.cmd(BrokerCmd::RecordFrames(false));
    rig.stop();
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn raw_outcome_is_reported() {
    let rig = Rig::start(None);
    rig.cmd(raw_command("t1", "22 00", "").unwrap());
    assert_eq!(
        rig.raw_event(),
        RawEvent {
            ok: true,
            detail: "queued".into()
        }
    );
    // Past the panel's own check (it refuses this too): the session's deny-list.
    rig.cmd(BrokerCmd::Raw {
        table: Table::T1,
        bytes: vec![0x24, 0x03, 0x01],
        confirmed_set: true,
    });
    let refused = rig.raw_event();
    assert!(!refused.ok);
    assert!(refused.detail.contains("denied"), "{refused:?}");
    // A SET that skipped the confirmation.
    rig.cmd(BrokerCmd::Raw {
        table: Table::T1,
        bytes: vec![0x58, 0x03, 0x01],
        confirmed_set: false,
    });
    let unconfirmed = rig.raw_event();
    assert!(!unconfirmed.ok && unconfirmed.detail.contains("confirmation"));
    rig.stop();
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
    raw: Receiver<RawEvent>,
    handle: JoinHandle<std::io::Result<()>>,
}

impl Rig {
    fn start(log_dir: Option<PathBuf>) -> Rig {
        let (tx, rx) = channel();
        let (ftx, frames) = channel();
        let (rtx, recording) = channel();
        let (stx, states) = channel();
        let (raw_tx, raw) = channel();
        let log_dir = log_dir.unwrap_or_else(|| std::env::temp_dir().join("shdc-unused"));
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
            let emit_raw = move |e: &RawEvent| {
                let _ = raw_tx.send(e.clone());
            };
            run_broker_logged(
                Box::new(FakeDevice::ult_wear()),
                rx,
                move |s: &DeviceState| {
                    let _ = stx.send(s.connected);
                },
                monotonic_ms,
                BrokerContext {
                    frames: Some(&mut tap),
                    raw: Some(&emit_raw),
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
            raw,
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

    fn raw_event(&self) -> RawEvent {
        self.raw
            .recv_timeout(Duration::from_secs(5))
            .expect("raw event before the deadline")
    }

    fn stop(self) {
        self.tx.send(BrokerCmd::Disconnect).unwrap();
        assert!(self.handle.join().unwrap().is_ok());
    }
}
