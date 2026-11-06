//! Tauri commands: thin forwards to the broker channel.

use std::sync::mpsc::Sender;
use std::sync::Mutex;

use mdr_proto::{NcAsmState, NcMode, UltLevel};
use mdr_session::DeviceState;
use tauri::{AppHandle, State};

use crate::broker::BrokerCmd;
use crate::popover;

/// The broker's command channel, held in Tauri state.
pub struct Broker(pub Sender<BrokerCmd>);

impl Broker {
    fn send(&self, cmd: BrokerCmd) -> Result<(), String> {
        self.0.send(cmd).map_err(|_| "the broker has stopped".to_string())
    }
}

#[tauri::command]
pub fn get_state(state: State<'_, Mutex<DeviceState>>) -> DeviceState {
    state.lock().map(|s| s.clone()).unwrap_or_default()
}

#[tauri::command]
pub fn set_nc(broker: State<'_, Broker>, mode: String, level: u8, voice_focus: bool) -> Result<(), String> {
    let mode = match mode.as_str() {
        "off" => NcMode::Off,
        "nc" => NcMode::NoiseCancelling,
        "ambient" => NcMode::Ambient,
        other => return Err(format!("unknown noise mode {other:?}")),
    };
    if !(1..=20).contains(&level) {
        return Err(format!("ambient level {level} is outside 1..=20"));
    }
    broker.send(BrokerCmd::SetNc(NcAsmState { mode, level, voice_focus }))
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

#[tauri::command]
pub fn connect(broker: State<'_, Broker>) -> Result<(), String> {
    broker.send(BrokerCmd::Connect)
}

#[tauri::command]
pub fn open_main_window(app: AppHandle) -> Result<(), String> {
    popover::open_main_window(&app).map_err(|e| e.to_string())
}
