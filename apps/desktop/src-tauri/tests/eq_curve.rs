//! The Advanced curve store and sending its best fit through the EQ path.

use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use mdr_frame::FrameType;
use mdr_proto::eq::{best_fit, to_raw};
use mdr_proto::{EqUltCapability, UltLevel};
use mdr_session::{DeviceState, Status};
use mdr_transport::FakeDevice;
use shdc_lib::broker::{run_broker, BrokerCmd};
use shdc_lib::eq_curve::{
    check_curve, check_shape, check_target_preset, fit_command, load, prepare_preset_ult_bands,
    prepare_send, save, EqCurve,
};

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("shdc-eqcurve-{name}-{}", std::process::id()));
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
    assert!(prepare_send(Some(&cap(10, 21)), &curve, MANUAL).is_err());
    assert!(prepare_send(None, &[0; 9], MANUAL).is_err());
    let (saved, fitted, cmd) = prepare_send(Some(&cap(6, 21)), &curve, MANUAL).unwrap();
    assert_eq!(saved.bands, curve);
    assert_eq!(fitted, [5, 1, -2, 1, 3, 6]);
    assert_eq!(
        cmd,
        BrokerCmd::SetEqPresetBands(MANUAL, vec![15, 11, 8, 11, 13, 16])
    );
}

const MANUAL: u8 = 0xA0;
const CUSTOM_1: u8 = 0xA1;
const CUSTOM_2: u8 = 0xA2;
const BRIGHT: u8 = 0x10;

#[test]
fn send_best_fit_only_targets_manual_or_custom() {
    let curve = [0; 10];
    for preset in [MANUAL, CUSTOM_1, CUSTOM_2] {
        assert_eq!(check_target_preset(preset), Ok(preset));
        assert!(prepare_send(None, &curve, preset).is_ok());
    }
    for preset in [0x00, BRIGHT, 0x16, 0x17, 0xA3, 0xFF] {
        let err = "best fit can only be written into Manual, Custom 1 or Custom 2".to_string();
        assert_eq!(check_target_preset(preset), Err(err.clone()));
        assert_eq!(prepare_send(None, &curve, preset).map(|_| ()), Err(err));
    }
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
fn send_best_fit_needs_the_target_listed() {
    let curve = [0; 10];
    let mut listed = cap(6, 21);
    listed.presets = vec![
        (0x00, String::new()),
        (BRIGHT, String::new()),
        (MANUAL, String::new()),
    ];
    assert!(prepare_send(Some(&listed), &curve, MANUAL).is_ok());
    assert_eq!(
        prepare_send(Some(&listed), &curve, CUSTOM_1).map(|_| ()),
        Err("the headphones do not list Custom 1".to_string())
    );
    assert_eq!(
        prepare_send(Some(&listed), &curve, CUSTOM_2).map(|_| ()),
        Err("the headphones do not list Custom 2".to_string())
    );
    // An empty list means the presets are not known yet: allowed.
    assert!(prepare_send(Some(&cap(6, 21)), &curve, CUSTOM_1).is_ok());
}

#[test]
fn send_best_fit_writes_preset_and_bands_in_one_frame() {
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
    // A fixed preset (Bright) and ULT 2: the fit goes into Custom 1, ULT kept.
    tx.send(BrokerCmd::SetEqPreset(BRIGHT)).unwrap();
    wait_for(&erx, |s| {
        s.eq_ult.value.as_ref().map(|e| e.preset) == Some(BRIGHT)
            && matches!(s.eq_ult.status, Status::Confirmed)
    });
    tx.send(BrokerCmd::SetUlt(UltLevel::Ult2)).unwrap();
    let ready = wait_for(&erx, |s| {
        s.eq_ult.value.as_ref().map(|e| (e.preset, e.ult)) == Some((BRIGHT, UltLevel::Ult2))
            && matches!(s.eq_ult.status, Status::Confirmed)
    });

    let curve = EqCurve {
        bands: [8, 6, 4, 2, 0, -2, 0, 2, 4, 6],
    };
    assert_eq!(
        ready.eq_capability.as_ref().map(|c| (c.bands, c.steps)),
        Some((6, 21))
    );
    let (_, sent_fit, sent_cmd) =
        prepare_send(ready.eq_capability.as_ref(), &curve.bands, CUSTOM_1).unwrap();
    let (fitted, cmd) = fit_command(&curve, CUSTOM_1);
    assert_eq!((sent_fit, &sent_cmd), (fitted, &cmd));
    assert_eq!(fitted, best_fit(&curve.bands));
    assert_eq!(fitted, [5, 1, -2, 1, 3, 6]);
    let raw: Vec<u8> = fitted.iter().map(|&v| to_raw(v)).collect();
    assert_eq!(raw, [15, 11, 8, 11, 13, 16]);
    assert_eq!(cmd, BrokerCmd::SetEqPresetBands(CUSTOM_1, raw.clone()));

    let eq_sets = |log: &[(FrameType, Vec<u8>)]| -> Vec<Vec<u8>> {
        log.iter()
            .filter(|(t, p)| *t == FrameType::Command1 && p.first() == Some(&0x58))
            .map(|(_, p)| p.clone())
            .collect()
    };
    let before = eq_sets(&log.lock().unwrap()).len();
    tx.send(cmd).unwrap();
    let st = wait_for(&erx, |s| {
        s.eq_ult.value.as_ref().map(|e| e.bands.clone()) == Some(raw.clone())
            && matches!(s.eq_ult.status, Status::Confirmed)
    });
    let eq = st.eq_ult.value.unwrap();
    assert_eq!((eq.preset, eq.ult), (CUSTOM_1, UltLevel::Ult2));

    let mut want = vec![0x58, 0x03, CUSTOM_1, 0x02, 0x06];
    want.extend_from_slice(&raw);
    let sets = eq_sets(&log.lock().unwrap());
    // Exactly one frame for the send, and no separate preset write.
    assert_eq!(sets[before..].to_vec(), vec![want], "{sets:02X?}");
    tx.send(BrokerCmd::Disconnect).unwrap();
}

fn listed(cap_bands: u8, steps: u8, presets: &[u8]) -> EqUltCapability {
    let mut c = cap(cap_bands, steps);
    c.presets = presets.iter().map(|&id| (id, String::new())).collect();
    c
}

#[test]
fn preset_code_checks_preset_ult_and_bands() {
    let six = [6, 0, 20, 10, 13, 16];
    for preset in [MANUAL, CUSTOM_1, CUSTOM_2] {
        assert_eq!(
            prepare_preset_ult_bands(None, preset, 1, &six),
            Ok(BrokerCmd::SetEqPresetUltBands(
                preset,
                UltLevel::Ult1,
                six.to_vec()
            ))
        );
    }
    let only = "a preset code can only be written into Manual, Custom 1 or Custom 2".to_string();
    for preset in [0x00, BRIGHT, 0x17, 0xA3] {
        assert_eq!(
            prepare_preset_ult_bands(None, preset, 0, &six),
            Err(only.clone())
        );
    }
    assert_eq!(
        prepare_preset_ult_bands(Some(&listed(6, 21, &[0x00, MANUAL])), CUSTOM_2, 0, &six),
        Err("the headphones do not list Custom 2".to_string())
    );
    assert!(
        prepare_preset_ult_bands(Some(&listed(6, 21, &[MANUAL, CUSTOM_2])), CUSTOM_2, 0, &six)
            .is_ok()
    );
    assert_eq!(
        prepare_preset_ult_bands(None, MANUAL, 3, &six),
        Err("unknown ULT level 3".to_string())
    );
    assert_eq!(
        prepare_preset_ult_bands(None, MANUAL, 0, &six[..5]),
        Err("expected 6 bands, got 5".to_string())
    );
    assert_eq!(
        prepare_preset_ult_bands(None, MANUAL, 0, &[0, 0, 0, 0, 0, 21]),
        Err("band level 21 is outside 0..=20".to_string())
    );
    // The headphones' own shape wins over the defaults.
    assert_eq!(
        prepare_preset_ult_bands(Some(&cap(5, 11)), MANUAL, 0, &six),
        Err("expected 5 bands, got 6".to_string())
    );
    assert_eq!(
        prepare_preset_ult_bands(Some(&cap(6, 11)), MANUAL, 0, &six),
        Err("band level 20 is outside 0..=10".to_string())
    );
    assert!(prepare_preset_ult_bands(Some(&cap(6, 11)), MANUAL, 0, &[0, 1, 2, 3, 4, 10]).is_ok());
}

#[test]
fn preset_code_writes_preset_ult_and_bands_in_one_frame() {
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
    let ready = wait_for(&erx, |s| {
        s.connected && s.eq_capability.is_some() && matches!(s.eq_ult.status, Status::Confirmed)
    });
    let bands = vec![6, 0, 20, 10, 13, 16];
    let cmd = prepare_preset_ult_bands(ready.eq_capability.as_ref(), CUSTOM_2, 1, &bands).unwrap();
    let eq_sets = |log: &[(FrameType, Vec<u8>)]| -> Vec<Vec<u8>> {
        log.iter()
            .filter(|(t, p)| *t == FrameType::Command1 && p.first() == Some(&0x58))
            .map(|(_, p)| p.clone())
            .collect()
    };
    let before = eq_sets(&log.lock().unwrap()).len();
    tx.send(cmd).unwrap();
    let st = wait_for(&erx, |s| {
        s.eq_ult.value.as_ref().map(|e| e.bands.clone()) == Some(bands.clone())
            && matches!(s.eq_ult.status, Status::Confirmed)
    });
    let eq = st.eq_ult.value.unwrap();
    assert_eq!((eq.preset, eq.ult), (CUSTOM_2, UltLevel::Ult1));
    let sets = eq_sets(&log.lock().unwrap());
    assert_eq!(
        sets[before..].to_vec(),
        vec![vec![0x58, 0x03, 0xA2, 0x01, 0x06, 6, 0, 20, 10, 13, 16]],
        "{sets:02X?}"
    );
    tx.send(BrokerCmd::Disconnect).unwrap();
}
