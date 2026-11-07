//! The Advanced curve store and sending its best fit through the EQ path.

use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use hairline_lib::broker::{run_broker, BrokerCmd};
use hairline_lib::eq_curve::{
    check_curve, check_shape, fit_command, load, prepare_send, save, EqCurve,
};
use mdr_frame::FrameType;
use mdr_proto::eq::{best_fit, to_raw};
use mdr_proto::{EqUltCapability, UltLevel};
use mdr_session::{DeviceState, Status};
use mdr_transport::FakeDevice;

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("hairline-eqcurve-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

fn monotonic_ms() -> u64 {
    static START: OnceLock<Instant> = OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_millis() as u64
}

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
fn round_trip() {
    let dir = temp_dir("round-trip");
    assert_eq!(load(&dir), EqCurve::default());
    assert_eq!(EqCurve::default().bands, [0; 10]);
    assert!(!dir.exists(), "load must not write");
    let curve = EqCurve {
        bands: [8, 6, 4, 2, 0, -2, 0, 2, 4, -10],
    };
    save(&dir, &curve).unwrap();
    assert_eq!(load(&dir), curve);
    assert!(!dir.join("eq_curve.json.tmp").exists());
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn corrupt_file_gives_default_and_is_kept() {
    for (name, text) in [
        ("garbage", "{not json"),
        ("short", r#"{"bands":[1,2,3]}"#),
        ("range", r#"{"bands":[0,0,0,0,0,0,0,0,0,11]}"#),
    ] {
        let dir = temp_dir(name);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("eq_curve.json"), text).unwrap();
        assert_eq!(load(&dir), EqCurve::default(), "{name}");
        assert!(!dir.join("eq_curve.json").exists(), "{name}");
        assert_eq!(
            std::fs::read_to_string(dir.join("eq_curve.json.bad")).unwrap(),
            text
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }
}

#[test]
fn unreadable_file_is_left_alone() {
    // A directory where the file should be: reading fails with something
    // other than NotFound or a parse error.
    let dir = temp_dir("unreadable");
    std::fs::create_dir_all(dir.join("eq_curve.json")).unwrap();
    assert_eq!(load(&dir), EqCurve::default());
    assert!(dir.join("eq_curve.json").is_dir(), "left in place");
    assert!(!dir.join("eq_curve.json.bad").exists(), "not renamed");
    std::fs::remove_dir_all(&dir).unwrap();
}

fn cap(bands: u8, steps: u8) -> EqUltCapability {
    EqUltCapability {
        bands,
        steps,
        ult2_additional_step: 0,
        presets: vec![],
    }
}

#[test]
fn send_best_fit_needs_six_bands_of_21_steps() {
    assert_eq!(check_shape(None), Ok(()));
    assert_eq!(check_shape(Some(&cap(6, 21))), Ok(()));
    assert_eq!(
        check_shape(Some(&cap(5, 21))),
        Err(
            "this headphone's equalizer has 5 bands / 21 steps; best fit needs 6 and 21"
                .to_string()
        )
    );
    assert_eq!(
        check_shape(Some(&cap(6, 11))),
        Err(
            "this headphone's equalizer has 6 bands / 11 steps; best fit needs 6 and 21"
                .to_string()
        )
    );
    let curve = [8, 6, 4, 2, 0, -2, 0, 2, 4, 6];
    assert!(prepare_send(Some(&cap(10, 21)), &curve).is_err());
    assert!(prepare_send(None, &[0; 9]).is_err());
    let (saved, fitted, cmd) = prepare_send(Some(&cap(6, 21)), &curve).unwrap();
    assert_eq!(saved.bands, curve);
    assert_eq!(fitted, [5, 1, -2, 1, 3, 6]);
    assert_eq!(cmd, BrokerCmd::SetEqBands(vec![15, 11, 8, 11, 13, 16]));
}

#[test]
fn rejects_out_of_range() {
    assert_eq!(
        check_curve(&[0, 1, 2, 3, 4, 5, 6, 7, 8, 9]),
        Ok([0, 1, 2, 3, 4, 5, 6, 7, 8, 9])
    );
    assert_eq!(check_curve(&[-10; 10]), Ok([-10; 10]));
    assert_eq!(check_curve(&[10; 10]), Ok([10; 10]));
    assert!(check_curve(&[0; 9]).is_err());
    assert!(check_curve(&[0; 11]).is_err());
    assert!(check_curve(&[]).is_err());
    let mut high = [0; 10];
    high[4] = 11;
    assert_eq!(
        check_curve(&high),
        Err("band level 11 is outside -10..=10".to_string())
    );
    let mut low = [0; 10];
    low[9] = -11;
    assert!(check_curve(&low).is_err());
}

#[test]
fn send_best_fit_uses_set_eq_bands() {
    let (tx, rx) = channel();
    let (etx, erx) = channel();
    let fake = FakeDevice::ult_wear();
    let log = fake.received_log();
    std::thread::spawn(move || {
        run_broker(
            Box::new(fake),
            rx,
            |s| etx.send(s.clone()).unwrap(),
            monotonic_ms,
        )
    });
    wait_for(&erx, |s| s.connected);
    // Move preset and ULT away from the fake's defaults so preserving them shows.
    tx.send(BrokerCmd::SetEqPreset(0x16)).unwrap();
    wait_for(&erx, |s| {
        s.eq_ult.value.as_ref().map(|e| e.preset) == Some(0x16)
            && matches!(s.eq_ult.status, Status::Confirmed)
    });
    tx.send(BrokerCmd::SetUlt(UltLevel::Ult2)).unwrap();
    let ready = wait_for(&erx, |s| {
        s.eq_ult.value.as_ref().map(|e| (e.preset, e.ult)) == Some((0x16, UltLevel::Ult2))
            && matches!(s.eq_ult.status, Status::Confirmed)
    });

    let curve = EqCurve {
        bands: [8, 6, 4, 2, 0, -2, 0, 2, 4, 6],
    };
    assert_eq!(
        ready.eq_capability.as_ref().map(|c| (c.bands, c.steps)),
        Some((6, 21))
    );
    let (_, sent_fit, sent_cmd) = prepare_send(ready.eq_capability.as_ref(), &curve.bands).unwrap();
    let (fitted, cmd) = fit_command(&curve);
    assert_eq!((sent_fit, &sent_cmd), (fitted, &cmd));
    assert_eq!(fitted, best_fit(&curve.bands));
    assert_eq!(fitted, [5, 1, -2, 1, 3, 6]);
    let raw: Vec<u8> = fitted.iter().map(|&v| to_raw(v)).collect();
    assert_eq!(raw, [15, 11, 8, 11, 13, 16]);
    assert_eq!(cmd, BrokerCmd::SetEqBands(raw.clone()));
    tx.send(cmd).unwrap();
    let st = wait_for(&erx, |s| {
        s.eq_ult.value.as_ref().map(|e| e.bands.clone()) == Some(raw.clone())
            && matches!(s.eq_ult.status, Status::Confirmed)
    });
    let eq = st.eq_ult.value.unwrap();
    assert_eq!((eq.preset, eq.ult), (0x16, UltLevel::Ult2));

    let mut want = vec![0x58, 0x03, 0x16, 0x02, 0x06];
    want.extend_from_slice(&raw);
    let frames = log.lock().unwrap().clone();
    let sets: Vec<&Vec<u8>> = frames
        .iter()
        .filter(|(t, p)| *t == FrameType::Command1 && p.first() == Some(&0x58))
        .map(|(_, p)| p)
        .collect();
    assert_eq!(sets.last(), Some(&&want), "{sets:02X?}");
    tx.send(BrokerCmd::Disconnect).unwrap();
}
