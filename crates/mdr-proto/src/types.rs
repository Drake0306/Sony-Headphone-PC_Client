use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Table {
    T1,
    T2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NcMode {
    Off,
    NoiseCancelling,
    Ambient,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct NcAsmState {
    pub mode: NcMode,
    /// Ambient level, 1..=20.
    pub level: u8,
    pub voice_focus: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UltLevel {
    Off = 0,
    Ult1 = 1,
    Ult2 = 2,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EqUltState {
    pub preset: u8,
    pub ult: UltLevel,
    pub bands: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EqUltCapability {
    pub bands: u8,
    pub steps: u8,
    pub ult2_additional_step: u8,
    pub presets: Vec<(u8, String)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Codec {
    Unsettled,
    Sbc,
    Aac,
    Ldac,
    Lc3,
    Other(u8),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Charging {
    NotCharging,
    Charging,
    Unknown,
    Charged,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeviceInfo {
    ModelName(String),
    Firmware(String),
    SeriesAndColor { series: u8, color: u8 },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SupportFunction {
    pub id: u8,
    pub priority: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GsKind {
    Boolean,
    List,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GsCapability {
    pub setting: u8,
    pub kind: GsKind,
    pub subject: String,
    pub summary: String,
    pub elements: Vec<(String, String)>,
}
