//! Tauri commands: thin forwards to the broker channel.

use std::sync::mpsc::Sender;
use std::sync::Mutex;

use mdr_proto::{preset_name, NcAsmState, NcMode, UltLevel};
use mdr_session::DeviceState;
use serde::Serialize;
use serde_json::Value;
use tauri::{AppHandle, Emitter, State};

use crate::battery_history::{self, BatteryEstimate};
use crate::broker::{raw_command, BrokerCmd, RecordingEvent};
use crate::device_record::{capability_rows, CapabilityRow};
use crate::frame_log::CaptureDir;
use crate::frame_ring::Recorder;
use crate::popover;
use crate::scenes::model::Scene;
use crate::scenes::store;
use crate::scenes::validate::check_scenes;
use crate::settings;

/// The broker's command channel, held in Tauri state.
pub struct Broker(pub Sender<BrokerCmd>);

impl Broker {
    pub(crate) fn send(&self, cmd: BrokerCmd) -> Result<(), String> {
        self.0
            .send(cmd)
            .map_err(|_| "the broker has stopped".to_string())
    }
}

/// How this run was started, held in Tauri state.
pub struct Runtime {
    pub fake: bool,
}

/// What the UI needs to know about this run.
#[derive(Debug, Clone, Serialize)]
pub struct RuntimeInfo {
    /// True when the scripted fake headphones stand in for the real ones.
    pub fake: bool,
    pub version: String,
}

#[tauri::command]
pub fn runtime_info(app: AppHandle, runtime: State<'_, Runtime>) -> RuntimeInfo {
    RuntimeInfo {
        fake: runtime.fake,
        version: app.package_info().version.to_string(),
    }
}

#[tauri::command]
pub fn get_state(state: State<'_, Mutex<DeviceState>>) -> DeviceState {
    state.lock().map(|s| s.clone()).unwrap_or_default()
}

#[tauri::command]
pub fn set_nc(
    broker: State<'_, Broker>,
    mode: String,
    level: u8,
    voice_focus: bool,
) -> Result<(), String> {
    broker.send(BrokerCmd::SetNc(NcAsmState {
        mode: nc_mode(&mode)?,
        level: check_level(level)?,
        voice_focus,
    }))
}

fn nc_mode(mode: &str) -> Result<NcMode, String> {
    match mode {
        "off" => Ok(NcMode::Off),
        "nc" => Ok(NcMode::NoiseCancelling),
        "ambient" => Ok(NcMode::Ambient),
        other => Err(format!("unknown noise mode {other:?}")),
    }
}

fn check_level(level: u8) -> Result<u8, String> {
    if (1..=20).contains(&level) {
        Ok(level)
    } else {
        Err(format!("ambient level {level} is outside 1..=20"))
    }
}

#[tauri::command]
pub fn set_ult(broker: State<'_, Broker>, level: u8) -> Result<(), String> {
    broker.send(BrokerCmd::SetUlt(ult_level(level)?))
}

fn ult_level(level: u8) -> Result<UltLevel, String> {
    match level {
        0 => Ok(UltLevel::Off),
        1 => Ok(UltLevel::Ult1),
        2 => Ok(UltLevel::Ult2),
        other => Err(format!("unknown ULT level {other}")),
    }
}

#[tauri::command]
pub fn set_multipoint(broker: State<'_, Broker>, on: bool) -> Result<(), String> {
    broker.send(BrokerCmd::SetMultipoint(on))
}

/// EQ preset ids the device offers (Off, the eight named presets, Manual, Custom 1/2).
pub const EQ_PRESETS: [u8; 12] = [
    0x00, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0xA0, 0xA1, 0xA2,
];
const EQ_BANDS: usize = 6;
const EQ_BAND_MAX: u8 = 20;
pub const VOLUME_MAX: u8 = 30;
/// Auto power-off: 5, 15, 30, 60, 180 min, when removed from ears, disabled.
const AUTO_POWER_OFF_VALUES: [u8; 7] = [0x00, 0x04, 0x01, 0x02, 0x03, 0x10, 0x11];
/// Voice assistant: phone default, Google, Alexa, Sony, none.
const VOICE_ASSISTANTS: [u8; 5] = [0x30, 0x31, 0x32, 0x34, 0xFF];

#[tauri::command]
pub fn set_eq_preset(broker: State<'_, Broker>, preset: u8) -> Result<(), String> {
    broker.send(BrokerCmd::SetEqPreset(check_eq_preset(preset)?))
}

fn check_eq_preset(preset: u8) -> Result<u8, String> {
    if EQ_PRESETS.contains(&preset) {
        Ok(preset)
    } else {
        Err(format!("unknown EQ preset {preset:#04X}"))
    }
}

#[tauri::command]
pub fn set_eq_bands(broker: State<'_, Broker>, bands: Vec<u8>) -> Result<(), String> {
    check_bands(&bands)?;
    broker.send(BrokerCmd::SetEqBands(bands))
}

fn check_bands(bands: &[u8]) -> Result<(), String> {
    if bands.len() != EQ_BANDS {
        return Err(format!("expected {EQ_BANDS} bands, got {}", bands.len()));
    }
    if let Some(b) = bands.iter().find(|&&b| b > EQ_BAND_MAX) {
        return Err(format!("band level {b} is outside 0..={EQ_BAND_MAX}"));
    }
    Ok(())
}

/// Writes a pasted Sound share code: `bands` (raw) and `ult` into `preset`
/// (Manual, Custom 1 or Custom 2) in one EQ frame. The UI calls it only after
/// the confirmation dialog.
#[tauri::command]
pub fn set_eq_preset_ult_bands(
    broker: State<'_, Broker>,
    device: State<'_, Mutex<DeviceState>>,
    preset: u8,
    ult: u8,
    bands: Vec<u8>,
) -> Result<(), String> {
    let cap = device
        .lock()
        .map_err(|_| "the device state is unavailable".to_string())?
        .eq_capability
        .clone();
    broker.send(crate::eq_curve::prepare_preset_ult_bands(
        cap.as_ref(),
        preset,
        ult,
        &bands,
    )?)
}

#[tauri::command]
pub fn set_volume(broker: State<'_, Broker>, volume: u8) -> Result<(), String> {
    broker.send(BrokerCmd::SetVolume(check_volume(volume)?))
}

fn check_volume(volume: u8) -> Result<u8, String> {
    if volume <= VOLUME_MAX {
        Ok(volume)
    } else {
        Err(format!("volume {volume} is outside 0..={VOLUME_MAX}"))
    }
}

/// Controls the local socket may set (`control_command`).
pub const SOCKET_CONTROLS: [&str; 7] = [
    "nc",
    "ambient_level",
    "voice_focus",
    "ult",
    "volume",
    "eq_preset",
    "dsee_auto_off",
];

/// One control from the local socket, validated with the same checks as the
/// `set_*` commands. `value` is the control's value; for `nc` it is the mode
/// (`"nc"`, `"ambient"`, `"off"`) or `{"mode", "level"?, "voice"?}`. The
/// noise switch, ambient level and voice focus share one SET, so the rest of
/// the NC/ASM state comes from `current`, as the window does (`noise.ts`).
pub fn control_command(
    control: &str,
    value: &Value,
    current: &DeviceState,
) -> Result<BrokerCmd, String> {
    let nc = current.nc.value;
    let cur_level = nc.map_or(1, |n| n.level.clamp(1, 20));
    let cur_voice = nc.is_some_and(|n| n.voice_focus);
    match control {
        "nc" => {
            let (mode, level, voice) = match value {
                Value::String(mode) => (mode.as_str(), None, None),
                Value::Object(o) => (
                    o.get("mode")
                        .and_then(Value::as_str)
                        .ok_or("nc needs a mode: nc, ambient or off")?,
                    o.get("level").filter(|v| !v.is_null()),
                    o.get("voice").filter(|v| !v.is_null()),
                ),
                _ => return Err("nc needs a mode: nc, ambient or off".into()),
            };
            let mode = nc_mode(mode)?;
            let level = match level {
                Some(v) => check_level(number(v, "ambient level", "1..=20")?)?,
                None => cur_level,
            };
            // Voice focus belongs to Ambient; asking for it elsewhere is refused.
            let voice_focus = match voice {
                Some(_) if mode != NcMode::Ambient => {
                    return Err("voice focus needs Ambient".into())
                }
                Some(v) => boolean(v, "voice")?,
                None => cur_voice,
            };
            Ok(BrokerCmd::SetNc(NcAsmState {
                mode,
                level,
                voice_focus,
            }))
        }
        "ambient_level" => Ok(BrokerCmd::SetNc(NcAsmState {
            mode: NcMode::Ambient,
            level: check_level(number(value, "ambient level", "1..=20")?)?,
            voice_focus: cur_voice,
        })),
        "voice_focus" => {
            let on = boolean(value, "voice focus")?;
            if nc.map(|n| n.mode) != Some(NcMode::Ambient) {
                return Err("voice focus needs Ambient".into());
            }
            Ok(BrokerCmd::SetNc(NcAsmState {
                mode: NcMode::Ambient,
                level: cur_level,
                voice_focus: on,
            }))
        }
        "ult" => Ok(BrokerCmd::SetUlt(ult_level(number(
            value,
            "ULT level",
            "0..=2",
        )?)?)),
        "volume" => Ok(BrokerCmd::SetVolume(check_volume(number(
            value, "volume", "0..=30",
        )?)?)),
        "eq_preset" => Ok(BrokerCmd::SetEqPreset(check_eq_preset(eq_preset(value)?)?)),
        "dsee_auto_off" => Ok(BrokerCmd::SetDseeAutoOff(boolean(value, "dsee_auto_off")?)),
        other => Err(format!(
            "unknown control {other:?} (one of {})",
            SOCKET_CONTROLS.join(", ")
        )),
    }
}

/// A whole number that fits a byte; anything else names the range.
fn number(value: &Value, what: &str, range: &str) -> Result<u8, String> {
    match value.as_u64().or_else(|| value.as_i64().map(|_| u64::MAX)) {
        Some(n) => u8::try_from(n).map_err(|_| format!("{what} {value} is outside {range}")),
        None => Err(format!("{what} needs a number in {range}")),
    }
}

fn boolean(value: &Value, what: &str) -> Result<bool, String> {
    value
        .as_bool()
        .ok_or_else(|| format!("{what} needs true or false"))
}

/// An EQ preset by id (a number, or hex like `"16"` / `"0x16"`) or by name
/// (`"Bass Boost"`, any case).
fn eq_preset(value: &Value) -> Result<u8, String> {
    if let Some(n) = value.as_u64() {
        return u8::try_from(n).map_err(|_| format!("unknown EQ preset {value}"));
    }
    let text = value
        .as_str()
        .ok_or("eq_preset needs a preset name or hex id")?
        .trim();
    if let Some(&id) = EQ_PRESETS
        .iter()
        .find(|&&id| preset_name(id).eq_ignore_ascii_case(text))
    {
        return Ok(id);
    }
    let hex = text
        .strip_prefix("0x")
        .or_else(|| text.strip_prefix("0X"))
        .unwrap_or(text);
    u8::from_str_radix(hex, 16).map_err(|_| format!("unknown EQ preset {text:?}"))
}

#[tauri::command]
pub fn set_dsee_auto_off(broker: State<'_, Broker>, auto: bool) -> Result<(), String> {
    broker.send(BrokerCmd::SetDseeAutoOff(auto))
}

#[tauri::command]
pub fn set_connection_mode(broker: State<'_, Broker>, value: u8) -> Result<(), String> {
    if value > 1 {
        return Err(format!("connection mode {value} is not 0 or 1"));
    }
    broker.send(BrokerCmd::SetConnectionMode(value))
}

#[tauri::command]
pub fn set_auto_power_off(broker: State<'_, Broker>, value: u8) -> Result<(), String> {
    if !AUTO_POWER_OFF_VALUES.contains(&value) {
        return Err(format!("unknown auto power-off value {value:#04X}"));
    }
    broker.send(BrokerCmd::SetAutoPowerOff(value))
}

#[tauri::command]
pub fn set_wear_pause(broker: State<'_, Broker>, on: bool) -> Result<(), String> {
    broker.send(BrokerCmd::SetWearPause(on))
}

#[tauri::command]
pub fn set_voice_assistant(broker: State<'_, Broker>, value: u8) -> Result<(), String> {
    if !VOICE_ASSISTANTS.contains(&value) {
        return Err(format!("unknown voice assistant {value:#04X}"));
    }
    broker.send(BrokerCmd::SetVoiceAssistant(value))
}

#[tauri::command]
pub fn set_quick_access(broker: State<'_, Broker>, functions: Vec<u8>) -> Result<(), String> {
    if functions.len() > usize::from(u8::MAX) {
        return Err(format!(
            "{} quick access functions is too many",
            functions.len()
        ));
    }
    broker.send(BrokerCmd::SetQuickAccess(functions))
}

#[tauri::command]
pub fn set_voice_guide_volume(broker: State<'_, Broker>, value: i8) -> Result<(), String> {
    if !(-2..=2).contains(&value) {
        return Err(format!("voice guide volume {value} is outside -2..=2"));
    }
    broker.send(BrokerCmd::SetVoiceGuideVolume(value))
}

#[tauri::command]
pub fn set_touch_panel(broker: State<'_, Broker>, on: bool) -> Result<(), String> {
    broker.send(BrokerCmd::SetTouchPanel(on))
}

#[tauri::command]
pub fn set_sidetone(broker: State<'_, Broker>, on: bool) -> Result<(), String> {
    broker.send(BrokerCmd::SetSidetone(on))
}

/// The list; also asks the broker to re-emit `"scenes"` so a newly opened
/// window learns the active scene.
#[tauri::command]
pub fn get_scenes(broker: State<'_, Broker>, scenes: State<'_, Mutex<Vec<Scene>>>) -> Vec<Scene> {
    let list = scenes.lock().map(|s| s.clone()).unwrap_or_default();
    let _ = broker.send(BrokerCmd::EmitScenes);
    list
}

/// Validates and saves the whole list, re-registers the hotkeys, then hands
/// the list to the broker, which emits the `"scenes"` event (with any hotkey
/// errors).
#[tauri::command]
pub fn set_scenes(
    app: AppHandle,
    broker: State<'_, Broker>,
    state: State<'_, Mutex<Vec<Scene>>>,
    scenes: Vec<Scene>,
) -> Result<(), String> {
    check_scenes(&scenes)?;
    let dir = settings::dir(&app).map_err(|e| e.to_string())?;
    // Held across save, update and send so concurrent saves cannot interleave.
    let mut current = state
        .lock()
        .map_err(|_| "scene list unavailable".to_string())?;
    store::save(&dir, &scenes).map_err(|e| format!("could not save scenes: {e}"))?;
    *current = scenes.clone();
    crate::hotkeys::apply(&app, &scenes);
    broker.send(BrokerCmd::SetScenes(scenes))
}

#[tauri::command]
pub fn apply_scene(
    broker: State<'_, Broker>,
    state: State<'_, Mutex<Vec<Scene>>>,
    id: String,
) -> Result<(), String> {
    let known = state
        .lock()
        .map(|s| s.iter().any(|scene| scene.id == id))
        .unwrap_or(false);
    if !known {
        return Err(format!("no scene {id:?}"));
    }
    broker.send(BrokerCmd::ApplyScene(id))
}

#[tauri::command]
pub fn connect(broker: State<'_, Broker>) -> Result<(), String> {
    broker.send(BrokerCmd::Connect)
}

/// Opens the main window. Async on purpose: a synchronous command runs inside
/// the calling webview's message callback, and on Windows creating a second
/// WebView2 there (which spins a nested message loop while the controller
/// comes up) left the new window white. From the async runtime the work is
/// posted to the main thread as its own event-loop task.
#[tauri::command]
pub async fn open_main_window(app: AppHandle) -> Result<(), String> {
    let (tx, rx) = std::sync::mpsc::channel();
    let handle = app.clone();
    app.run_on_main_thread(move || {
        let _ = tx.send(popover::open_main_window(&handle).map_err(|e| e.to_string()));
    })
    .map_err(|e| e.to_string())?;
    tauri::async_runtime::spawn_blocking(move || rx.recv())
        .await
        .map_err(|e| e.to_string())?
        .map_err(|_| "the main window was not created".to_string())?
}

/// The web page's own uncaught errors, written to the diagnostics log so a
/// blank window leaves a trace.
#[tauri::command]
pub fn report_frontend_error(app: AppHandle, window: tauri::Window, message: String) {
    let text: String = message.chars().take(4000).collect();
    crate::diag::log(&app, &format!("page error in {}: {text}", window.label()));
}

/// Every declared or proven support function of the connected headphones.
#[tauri::command]
pub fn capability_list(state: State<'_, Mutex<DeviceState>>) -> Vec<CapabilityRow> {
    state
        .lock()
        .map(|s| capability_rows(&s))
        .unwrap_or_default()
}

/// Starts or stops the `"frames"` event stream.
#[tauri::command]
pub fn watch_frames(broker: State<'_, Broker>, on: bool) -> Result<(), String> {
    broker.send(BrokerCmd::WatchFrames(on))
}

/// Starts or stops recording frames to the capture directory; returns the
/// file being written (`None` once stopped) and emits `"recording"`.
#[tauri::command]
pub fn record_frames(
    app: AppHandle,
    recorder: State<'_, Recorder>,
    capture_dir: State<'_, CaptureDir>,
    on: bool,
) -> Result<Option<String>, String> {
    let path = recorder
        .set(&capture_dir.0, on)
        .map_err(|e| format!("could not record frames: {e}"))?;
    let event = RecordingEvent::new(path.as_deref());
    let _ = app.emit("recording", event.clone());
    Ok(event.path)
}

/// Sends a hand-written request from the Advanced panel (`table` is `"t1"`
/// or `"t2"`, `hex` the payload). Anything but a GET needs `confirm` to be
/// `SEND`; the session's deny-list still applies. Refused unless the
/// Advanced panel is enabled in Settings and the headphones are connected.
/// `Ok` means handed to the broker; the `"raw"` event says whether the
/// session queued it.
#[tauri::command]
pub fn send_raw(
    broker: State<'_, Broker>,
    settings: State<'_, Mutex<settings::AppSettings>>,
    state: State<'_, Mutex<DeviceState>>,
    table: String,
    hex: String,
    confirm: String,
) -> Result<(), String> {
    let current = settings
        .lock()
        .map(|s| s.clone())
        .map_err(|_| "settings unavailable".to_string())?;
    settings::raw_allowed(&current)?;
    let connected = state
        .lock()
        .map(|s| s.connected)
        .map_err(|_| "state unavailable".to_string())?;
    if !connected {
        return Err("not connected".into());
    }
    broker.send(raw_command(&table, &hex, &confirm)?)
}

/// Starts the opcode probe (read-only GETs, one at a time). Refused unless
/// the Advanced panel is enabled, the headphones are connected and `confirm`
/// is `RUN`; progress and results arrive as `"probe"` events.
#[tauri::command]
pub fn probe_start(
    broker: State<'_, Broker>,
    settings: State<'_, Mutex<settings::AppSettings>>,
    state: State<'_, Mutex<DeviceState>>,
    confirm: String,
) -> Result<(), String> {
    let current = settings
        .lock()
        .map(|s| s.clone())
        .map_err(|_| "settings unavailable".to_string())?;
    let device = state
        .lock()
        .map(|s| s.clone())
        .map_err(|_| "state unavailable".to_string())?;
    crate::probe::probe_allowed(&current, &device, &confirm)?;
    broker.send(BrokerCmd::ProbeStart)
}

/// Stops the probe; the results so far are kept, saved and emitted.
#[tauri::command]
pub fn probe_stop(broker: State<'_, Broker>) -> Result<(), String> {
    broker.send(BrokerCmd::ProbeStop)
}

/// The battery history's estimate now, for a window that has just opened
/// (later ones arrive as `"battery"` events). Empty without a settings dir.
#[tauri::command]
pub fn battery_estimate(app: AppHandle) -> BatteryEstimate {
    settings::dir(&app)
        .map(|dir| {
            battery_history::estimate(&battery_history::load(&dir), battery_history::unix_now())
        })
        .unwrap_or_default()
}

/// Deletes `battery.jsonl` and emits the empty estimate; the broker then
/// takes a fresh sample if the headphones are connected.
#[tauri::command]
pub fn clear_battery_history(app: AppHandle, broker: State<'_, Broker>) -> Result<(), String> {
    let dir = settings::dir(&app).map_err(|e| e.to_string())?;
    battery_history::clear(&dir).map_err(|e| format!("battery history not cleared: {e}"))?;
    let _ = app.emit("battery", BatteryEstimate::default());
    broker.send(BrokerCmd::BatteryHistoryCleared)
}

#[cfg(test)]
mod tests {
    use super::{check_bands, control_command, BrokerCmd};
    use mdr_proto::{NcAsmState, NcMode};
    use mdr_session::{DeviceState, Status, Tracked};
    use serde_json::json;

    fn with_nc(mode: NcMode, level: u8, voice_focus: bool) -> DeviceState {
        DeviceState {
            nc: Tracked {
                value: Some(NcAsmState {
                    mode,
                    level,
                    voice_focus,
                }),
                status: Status::Confirmed,
            },
            ..DeviceState::default()
        }
    }

    fn nc(mode: NcMode, level: u8, voice_focus: bool) -> Result<BrokerCmd, String> {
        Ok(BrokerCmd::SetNc(NcAsmState {
            mode,
            level,
            voice_focus,
        }))
    }

    #[test]
    fn control_command_fills_nc_from_the_current_state() {
        let cur = with_nc(NcMode::Ambient, 9, true);
        assert_eq!(
            control_command("nc", &json!("nc"), &cur),
            nc(NcMode::NoiseCancelling, 9, true)
        );
        // Voice focus belongs to Ambient: asking for it elsewhere is refused.
        for mode in ["off", "nc"] {
            assert_eq!(
                control_command("nc", &json!({"mode": mode, "voice": false}), &cur),
                Err("voice focus needs Ambient".into())
            );
        }
        assert_eq!(
            control_command("ambient_level", &json!(3), &with_nc(NcMode::Off, 9, false)),
            nc(NcMode::Ambient, 3, false)
        );
        assert_eq!(
            control_command("voice_focus", &json!(false), &cur),
            nc(NcMode::Ambient, 9, false)
        );
        assert_eq!(
            control_command("voice_focus", &json!(true), &DeviceState::default()),
            Err("voice focus needs Ambient".into())
        );
        assert_eq!(
            control_command("nc", &json!({"mode": "ambient", "level": 21}), &cur),
            Err("ambient level 21 is outside 1..=20".into())
        );
        assert_eq!(
            control_command("nc", &json!("loud"), &cur),
            Err("unknown noise mode \"loud\"".into())
        );
    }

    #[test]
    fn control_command_checks_values_like_the_set_commands() {
        let cur = DeviceState::default();
        assert_eq!(
            control_command("volume", &json!(-1), &cur),
            Err("volume -1 is outside 0..=30".into())
        );
        assert_eq!(
            control_command("volume", &json!("loud"), &cur),
            Err("volume needs a number in 0..=30".into())
        );
        assert_eq!(
            control_command("ult", &json!(3), &cur),
            Err("unknown ULT level 3".into())
        );
        for preset in [json!("16"), json!("0x16"), json!(22), json!("BASS BOOST")] {
            assert_eq!(
                control_command("eq_preset", &preset, &cur),
                Ok(BrokerCmd::SetEqPreset(0x16)),
                "{preset}"
            );
        }
        assert_eq!(
            control_command("eq_preset", &json!("99"), &cur),
            Err("unknown EQ preset 0x99".into())
        );
        assert_eq!(
            control_command("dsee_auto_off", &json!(true), &cur),
            Ok(BrokerCmd::SetDseeAutoOff(true))
        );
        assert!(control_command("auto_power_off", &json!(0), &cur).is_err());
    }

    #[test]
    fn bands_need_six_levels_within_range() {
        assert!(check_bands(&[10, 13, 10, 14, 20, 0]).is_ok());
        assert!(check_bands(&[10, 10, 10, 10, 10]).is_err());
        assert!(check_bands(&[10, 10, 10, 10, 10, 21]).is_err());
    }
}
