//! The local socket: one JSON request per line, one JSON answer per line.
#![cfg(unix)]

use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use mdr_proto::{NcAsmState, NcMode, UltLevel};
use mdr_session::{DeviceState, Status, Tracked};
use serde_json::{json, Value};
use shdc_lib::battery_history::BatteryEstimate;
use shdc_lib::broker::BrokerCmd;
use shdc_lib::commands::control_command;
use shdc_lib::scenes::model::{Actions, Scene, Trigger};
use shdc_lib::socket::{bind, handle_line, serve, SocketContext};

#[derive(Default)]
struct Fake {
    state: DeviceState,
    scenes: Vec<Scene>,
    active: Option<String>,
    sent: Mutex<Vec<BrokerCmd>>,
}

impl SocketContext for Fake {
    fn state(&self) -> DeviceState {
        self.state.clone()
    }
    fn scenes(&self) -> (Vec<Scene>, Option<String>) {
        (self.scenes.clone(), self.active.clone())
    }
    fn battery(&self) -> BatteryEstimate {
        BatteryEstimate {
            minutes_left: Some(320),
            samples: 12,
            since: Some(1_700_000_000),
        }
    }
    fn send(&self, cmd: BrokerCmd) -> Result<(), String> {
        self.sent.lock().unwrap().push(cmd);
        Ok(())
    }
}

fn connected() -> Fake {
    let mut state = DeviceState {
        connected: true,
        ..DeviceState::default()
    };
    state.nc = Tracked {
        value: Some(NcAsmState {
            mode: NcMode::NoiseCancelling,
            level: 7,
            voice_focus: false,
        }),
        status: Status::Confirmed,
    };
    state.volume = Tracked {
        value: Some(12),
        status: Status::Confirmed,
    };
    Fake {
        state,
        scenes: vec![scene("s-1", "Commute"), scene("s-2", "Focus")],
        active: Some("s-2".into()),
        ..Fake::default()
    }
}

fn scene(id: &str, name: &str) -> Scene {
    Scene {
        id: id.into(),
        name: name.into(),
        enabled: true,
        trigger: Trigger::Manual,
        actions: Actions::default(),
        hotkey: None,
    }
}

fn ask(ctx: &Fake, line: &str) -> Value {
    serde_json::from_str(&handle_line(line, ctx)).expect("each answer is JSON")
}

#[test]
fn socket_rejects_bad_input_and_stays_up() {
    let ctx = connected();
    assert_eq!(
        ask(&ctx, "{not json"),
        json!({"ok": false, "error": "bad json"})
    );
    assert_eq!(
        ask(&ctx, "[1,2]"),
        json!({"ok": false, "error": "bad json"})
    );
    assert_eq!(
        ask(&ctx, r#"{"cmd":"dance"}"#),
        json!({"ok": false, "error": "unknown cmd"})
    );
    let unknown_control = ask(&ctx, r#"{"cmd":"set","control":"bass","value":3}"#);
    assert_eq!(unknown_control["ok"], json!(false));
    assert!(unknown_control["error"].is_string());
    assert_eq!(ask(&ctx, r#"{"cmd":"state"}"#)["ok"], json!(true));
    assert!(ctx.sent.lock().unwrap().is_empty());
}

#[test]
fn socket_state_round_trip() {
    let ctx = connected();
    let answer = ask(&ctx, r#"{"cmd":"state"}"#);
    assert_eq!(answer["ok"], json!(true));
    let state: DeviceState = serde_json::from_value(answer["state"].clone()).unwrap();
    assert_eq!(state, ctx.state);
    assert_eq!(answer["state"], serde_json::to_value(&ctx.state).unwrap());
}

#[test]
fn socket_set_uses_the_same_validation() {
    let ctx = connected();
    let answer = ask(&ctx, r#"{"cmd":"set","control":"volume","value":31}"#);
    let expected = control_command("volume", &json!(31), &ctx.state).unwrap_err();
    assert_eq!(expected, "volume 31 is outside 0..=30");
    assert_eq!(answer, json!({"ok": false, "error": expected}));

    assert_eq!(
        ask(&ctx, r#"{"cmd":"set","control":"ult","value":2}"#),
        json!({"ok": true})
    );
    assert_eq!(
        ask(
            &ctx,
            r#"{"cmd":"set","control":"eq_preset","value":"bass boost"}"#
        ),
        json!({"ok": true})
    );
    assert_eq!(
        ask(
            &ctx,
            r#"{"cmd":"set","control":"nc","value":"ambient","level":5,"voice":true}"#
        ),
        json!({"ok": true})
    );
    let voice = ask(
        &ctx,
        r#"{"cmd":"set","control":"voice_focus","value":true}"#,
    );
    assert_eq!(
        voice,
        json!({"ok": false, "error": "voice focus needs Ambient"})
    );
    assert_eq!(
        *ctx.sent.lock().unwrap(),
        vec![
            BrokerCmd::SetUlt(UltLevel::Ult2),
            BrokerCmd::SetEqPreset(0x16),
            BrokerCmd::SetNc(NcAsmState {
                mode: NcMode::Ambient,
                level: 5,
                voice_focus: true,
            }),
        ]
    );
}

#[test]
fn socket_set_needs_a_connection() {
    let ctx = Fake::default();
    assert_eq!(
        ask(&ctx, r#"{"cmd":"set","control":"volume","value":10}"#),
        json!({"ok": false, "error": "not connected"})
    );
    assert!(ctx.sent.lock().unwrap().is_empty());
}

#[test]
fn socket_never_exposes_raw_or_power() {
    let ctx = connected();
    for line in [
        r#"{"cmd":"raw","table":"t1","hex":"22 00 01"}"#,
        r#"{"cmd":"power"}"#,
        r#"{"cmd":"power_off"}"#,
        r#"{"cmd":"probe"}"#,
        r#"{"cmd":"reset"}"#,
    ] {
        assert_eq!(
            ask(&ctx, line),
            json!({"ok": false, "error": "unknown cmd"})
        );
    }
    for control in ["power", "raw", "reset", "auto_power_off"] {
        let answer = ask(
            &ctx,
            &json!({"cmd": "set", "control": control, "value": 0}).to_string(),
        );
        assert_eq!(answer["ok"], json!(false), "{control}");
    }
    assert!(ctx.sent.lock().unwrap().is_empty());
}

#[test]
fn socket_scenes_and_battery() {
    let ctx = connected();
    let scenes = ask(&ctx, r#"{"cmd":"scenes"}"#);
    assert_eq!(scenes["ok"], json!(true));
    assert_eq!(scenes["active"], json!("s-2"));
    assert_eq!(scenes["scenes"].as_array().unwrap().len(), 2);

    assert_eq!(
        ask(&ctx, r#"{"cmd":"scene","name":"commute"}"#),
        json!({"ok": true})
    );
    assert_eq!(
        ask(&ctx, r#"{"cmd":"scene","id":"s-2"}"#),
        json!({"ok": true})
    );
    assert_eq!(
        ask(&ctx, r#"{"cmd":"scene","name":"gym"}"#)["ok"],
        json!(false)
    );
    assert_eq!(
        *ctx.sent.lock().unwrap(),
        vec![
            BrokerCmd::ApplyScene("s-1".into()),
            BrokerCmd::ApplyScene("s-2".into())
        ]
    );

    let battery = ask(&ctx, r#"{"cmd":"battery"}"#);
    assert_eq!(
        battery,
        json!({"ok": true, "battery": {"minutes_left": 320, "samples": 12, "since": 1_700_000_000}})
    );
}

fn temp_sock(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("hl-sock-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir.join("shdc.sock")
}

#[test]
fn socket_bind_refuses_a_live_socket() {
    let path = temp_sock("live");
    let first = bind(&path).unwrap();
    let err = bind(&path).unwrap_err();
    assert!(
        err.to_string().contains("another SHDC is already serving"),
        "{err}"
    );
    // The first server's socket is untouched and still answers.
    let ctx: Arc<dyn SocketContext + Send + Sync> = Arc::new(connected());
    std::thread::spawn(move || serve(first, ctx));
    let stream = UnixStream::connect(&path).unwrap();
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let mut writer = stream;
    writer.write_all(b"{\"cmd\":\"state\"}\n").unwrap();
    let mut line = String::new();
    reader.read_line(&mut line).unwrap();
    assert!(line.contains("\"ok\":true"), "{line}");
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[test]
fn socket_bind_replaces_a_dead_socket() {
    let path = temp_sock("dead");
    drop(std::os::unix::net::UnixListener::bind(&path).unwrap());
    assert!(path.exists());
    assert!(bind(&path).is_ok());
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[test]
fn socket_listener_serves_a_real_client() {
    let path = temp_sock("real");
    // A stale file from a crashed run is replaced.
    std::fs::write(&path, b"stale").unwrap();
    let listener = bind(&path).unwrap();
    let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o600);
    let ctx: Arc<dyn SocketContext + Send + Sync> = Arc::new(connected());
    std::thread::spawn(move || serve(listener, ctx));

    let stream = UnixStream::connect(&path).unwrap();
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let mut writer = stream;
    let mut line = String::new();
    for (request, ok) in [
        ("garbage\n", false),
        ("\u{FF}\n", false),
        ("{\"cmd\":\"state\"}\n", true),
        ("{\"cmd\":\"battery\"}\n", true),
    ] {
        writer.write_all(request.as_bytes()).unwrap();
        line.clear();
        reader.read_line(&mut line).unwrap();
        let answer: Value = serde_json::from_str(&line).unwrap();
        assert_eq!(answer["ok"], json!(ok), "{request:?} -> {line}");
    }
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}
