use crate::opcode::{function, inquired, t1, t2};
use crate::types::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Group {
    Connect,
    Battery,
    NcAsm,
    EqUlt,
    Codec,
    Volume,
    GeneralSetting(u8),
    AutoPowerOff,
    Power,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    pub table: Table,
    pub bytes: Vec<u8>,
    /// Support-function id the device must declare before this is sent.
    pub capability: Option<u8>,
    pub group: Group,
}

fn t1_req(bytes: Vec<u8>, capability: Option<u8>, group: Group) -> Request {
    Request {
        table: Table::T1,
        bytes,
        capability,
        group,
    }
}

pub fn protocol_info() -> Request {
    t1_req(
        vec![t1::CONNECT_GET_PROTOCOL_INFO, 0x00],
        None,
        Group::Connect,
    )
}

pub fn support_functions(table: Table) -> Request {
    let op = match table {
        Table::T1 => t1::CONNECT_GET_SUPPORT_FUNCTION,
        Table::T2 => t2::CONNECT_GET_SUPPORT_FUNCTION,
    };
    Request {
        table,
        bytes: vec![op, 0x00],
        capability: None,
        group: Group::Connect,
    }
}

pub fn device_info(kind: u8) -> Request {
    t1_req(
        vec![t1::CONNECT_GET_DEVICE_INFO, kind],
        None,
        Group::Connect,
    )
}

pub fn battery_get() -> Request {
    t1_req(
        vec![t1::POWER_GET_STATUS, inquired::BATTERY_SINGLE],
        Some(function::BATTERY_LEVEL_INDICATOR),
        Group::Battery,
    )
}

const NCASM: u8 = inquired::NC_ASM_DUAL_NC_SWITCH_AND_ASM_SEAMLESS;

pub fn ncasm_get() -> Request {
    t1_req(
        vec![t1::NCASM_GET_PARAM, NCASM],
        Some(function::NC_ASM_DUAL_LEVEL_ADJUSTMENT),
        Group::NcAsm,
    )
}

pub fn ncasm_set(s: &NcAsmState) -> Request {
    let bytes = vec![
        t1::NCASM_SET_PARAM,
        NCASM,
        0x01, // valueChange: changed
        u8::from(s.mode != NcMode::Off),
        u8::from(s.mode == NcMode::Ambient),
        u8::from(s.voice_focus),
        s.level,
    ];
    t1_req(
        bytes,
        Some(function::NC_ASM_DUAL_LEVEL_ADJUSTMENT),
        Group::NcAsm,
    )
}

const EQ_ULT: u8 = inquired::PRESET_EQ_AND_ULT_MODE;

pub fn eq_ult_capability() -> Request {
    t1_req(
        vec![t1::EQEBB_GET_CAPABILITY, EQ_ULT],
        Some(function::PRESET_EQ_AND_ULT_MODE),
        Group::EqUlt,
    )
}

pub fn eq_ult_get() -> Request {
    t1_req(
        vec![t1::EQEBB_GET_PARAM, EQ_ULT],
        Some(function::PRESET_EQ_AND_ULT_MODE),
        Group::EqUlt,
    )
}

/// Callers must validate `bands` (1..=255 entries); a malformed SET can corrupt
/// the device's custom slot.
pub fn eq_ult_set(s: &EqUltState) -> Request {
    debug_assert!(!s.bands.is_empty() && s.bands.len() <= 255);
    let mut bytes = vec![
        t1::EQEBB_SET_PARAM,
        EQ_ULT,
        s.preset,
        s.ult as u8,
        s.bands.len() as u8,
    ];
    bytes.extend_from_slice(&s.bands);
    t1_req(bytes, Some(function::PRESET_EQ_AND_ULT_MODE), Group::EqUlt)
}

pub fn codec_get() -> Request {
    t1_req(
        vec![t1::COMMON_GET_STATUS, inquired::CODEC],
        Some(function::CODEC_INDICATOR),
        Group::Codec,
    )
}

pub fn volume_get() -> Request {
    t1_req(
        vec![t1::PLAY_GET_PARAM, inquired::VOLUME],
        Some(function::PLAYBACK_CONTROLLER),
        Group::Volume,
    )
}

pub fn volume_set(volume: u8) -> Request {
    t1_req(
        vec![t1::PLAY_SET_PARAM, inquired::VOLUME, volume],
        Some(function::PLAYBACK_CONTROLLER),
        Group::Volume,
    )
}

pub fn gs_capability(setting: u8) -> Request {
    t1_req(
        vec![t1::GS_GET_CAPABILITY, setting],
        Some(setting),
        Group::GeneralSetting(setting),
    )
}

pub fn gs_get(setting: u8) -> Request {
    t1_req(
        vec![t1::GS_GET_PARAM, setting],
        Some(setting),
        Group::GeneralSetting(setting),
    )
}

pub fn gs_set_bool(setting: u8, on: bool) -> Request {
    // Inverted enum: 0x00 is ON.
    let bytes = vec![
        t1::GS_SET_PARAM,
        setting,
        inquired::GS_BOOLEAN,
        u8::from(!on),
    ];
    t1_req(bytes, Some(setting), Group::GeneralSetting(setting))
}

pub fn gs_set_list(setting: u8, index: u8) -> Request {
    let bytes = vec![t1::GS_SET_PARAM, setting, inquired::GS_LIST, index];
    t1_req(bytes, Some(setting), Group::GeneralSetting(setting))
}

pub fn auto_power_off_get() -> Request {
    t1_req(
        vec![t1::POWER_GET_PARAM, inquired::AUTO_POWER_OFF],
        Some(function::AUTO_POWER_OFF),
        Group::AutoPowerOff,
    )
}

/// Sets both the current and last-selected value to `current`.
pub fn auto_power_off_set(current: u8) -> Request {
    let bytes = vec![
        t1::POWER_SET_PARAM,
        inquired::AUTO_POWER_OFF,
        current,
        current,
    ];
    t1_req(bytes, Some(function::AUTO_POWER_OFF), Group::AutoPowerOff)
}

pub fn power_off() -> Request {
    let bytes = vec![
        t1::POWER_SET_STATUS,
        inquired::POWER_OFF,
        inquired::POWER_OFF_USER,
    ];
    t1_req(bytes, Some(function::POWER_OFF), Group::Power)
}
