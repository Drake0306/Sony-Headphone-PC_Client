//! Tauri commands: thin forwards to the broker channel.

use std::sync::mpsc::Sender;
use std::sync::Mutex;

use mdr_proto::{NcAsmState, NcMode, UltLevel};
use mdr_session::DeviceState;
use serde::Serialize;
use tauri::{AppHandle, State};

use crate::broker::{parse_raw, BrokerCmd};
use crate::popover;
use crate::scenes::model::Scene;
use crate::scenes::store;
use crate::scenes::validate::check_scenes;
use crate::settings;

/// The broker's command channel, held in Tauri state.
pub struct Broker(pub Sender<BrokerCmd>);

impl Broker {
    fn send(&self, cmd: BrokerCmd) -> Result<(), String> {
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
    let mode = match mode.as_str() {
        "off" => NcMode::Off,
        "nc" => NcMode::NoiseCancelling,
        "ambient" => NcMode::Ambient,
        other => return Err(format!("unknown noise mode {other:?}")),
    };
    if !(1..=20).contains(&level) {
        return Err(format!("ambient level {level} is outside 1..=20"));
    }
    broker.send(BrokerCmd::SetNc(NcAsmState {
        mode,
        level,
        voice_focus,
    }))
}

#[tauri::command]
pub fn set_ult(broker: State<'_, Broker>, level: u8) -> Result<(), String> {
    let ult = match level {
        0 => UltLevel::Off,
        1 => UltLevel::Ult1,
        2 => UltLevel::Ult2,
        other => return Err(format!("unknown ULT level {other}")),
    };
    broker.send(BrokerCmd::SetUlt(ult))
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
    if !EQ_PRESETS.contains(&preset) {
        return Err(format!("unknown EQ preset {preset:#04X}"));
    }
    broker.send(BrokerCmd::SetEqPreset(preset))
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

#[tauri::command]
pub fn set_volume(broker: State<'_, Broker>, volume: u8) -> Result<(), String> {
    if volume > VOLUME_MAX {
        return Err(format!("volume {volume} is outside 0..={VOLUME_MAX}"));
    }
    broker.send(BrokerCmd::SetVolume(volume))
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

#[tauri::command]
pub fn open_main_window(app: AppHandle) -> Result<(), String> {
    popover::open_main_window(&app).map_err(|e| e.to_string())
}

/// Sends a hand-written request (`table` is `"t1"` or `"t2"`, `hex` the payload).
/// Registered only when `HAIRLINE_DEBUG=1`; the deny-list still applies.
#[tauri::command]
pub fn debug_raw(broker: State<'_, Broker>, table: String, hex: String) -> Result<(), String> {
    broker.send(parse_raw(&table, &hex)?)
}

#[cfg(test)]
mod tests {
    use super::check_bands;

    #[test]
    fn bands_need_six_levels_within_range() {
        assert!(check_bands(&[10, 13, 10, 14, 20, 0]).is_ok());
        assert!(check_bands(&[10, 10, 10, 10, 10]).is_err());
        assert!(check_bands(&[10, 10, 10, 10, 10, 21]).is_err());
    }
}
