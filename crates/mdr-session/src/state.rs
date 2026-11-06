//! Device state as the UI sees it, and how decoded messages update it.

use mdr_proto::{
    Charging, Codec, DeviceInfo, EqUltCapability, EqUltState, Group, GsCapability, Message,
    NcAsmState, Table,
};
use serde::{Deserialize, Serialize};

/// General setting that drives `DeviceState::multipoint`.
pub const GS_MULTIPOINT: u8 = 0xD1;
/// General setting that drives `DeviceState::touch_panel`.
pub const GS_TOUCH_PANEL: u8 = 0xD2;
/// General setting that drives `DeviceState::sidetone`.
pub const GS_SIDETONE: u8 = 0xD3;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Status {
    Confirmed,
    Pending { since_ms: u64 },
    Refused { reason: String },
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Tracked<T> {
    pub value: Option<T>,
    pub status: Status,
}

impl<T> Tracked<T> {
    pub fn unavailable() -> Self {
        Tracked {
            value: None,
            status: Status::Unavailable,
        }
    }

    pub fn pending(now_ms: u64) -> Self {
        Tracked {
            value: None,
            status: Status::Pending { since_ms: now_ms },
        }
    }

    fn confirm(&mut self, value: T) {
        self.value = Some(value);
        self.status = Status::Confirmed;
    }
}

impl<T> Default for Tracked<T> {
    fn default() -> Self {
        Self::unavailable()
    }
}

/// Current value of a general setting other than multipoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GsValue {
    Bool(bool),
    Index(u8),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GsSetting {
    pub capability: GsCapability,
    pub current: Option<GsValue>,
}

/// A device paired with the host; filled in by the app, not the session.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PairedDevice {
    pub name: String,
    pub playing: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct DeviceState {
    pub connected: bool,
    /// Bluetooth address of the linked headphones, as the transport reported it.
    pub address: Option<String>,
    /// RFCOMM channel of the v2 service, when the transport exposes it.
    pub channel: Option<u8>,
    pub model: Option<String>,
    pub firmware: Option<String>,
    pub color: Option<u8>,
    pub capabilities_t1: Vec<u8>,
    pub capabilities_t2: Vec<u8>,
    pub proven: Vec<(Table, u8)>,
    pub battery: Tracked<(u8, Charging)>,
    pub nc: Tracked<NcAsmState>,
    pub eq_ult: Tracked<EqUltState>,
    pub eq_capability: Option<EqUltCapability>,
    pub codec: Tracked<Codec>,
    pub volume: Tracked<u8>,
    pub multipoint: Tracked<bool>,
    pub auto_power_off: Tracked<u8>,
    pub dsee: Tracked<bool>,
    /// DSEE Extreme upscaling mode: true is Auto (upscales when it helps),
    /// false is Off (`UpscalingTypeAutoOff`).
    pub dsee_auto_off: Tracked<bool>,
    pub connection_mode: Tracked<u8>,
    pub wear_pause: Tracked<bool>,
    pub voice_assistant: Tracked<u8>,
    pub quick_access: Tracked<Vec<u8>>,
    pub voice_guide_volume: Tracked<i8>,
    /// Last reported `PlaybackStatus`; read-only, so not tracked.
    pub playback: Option<u8>,
    pub touch_panel: Tracked<bool>,
    pub sidetone: Tracked<bool>,
    pub gs: Vec<GsSetting>,
    pub devices: Vec<PairedDevice>,
}

/// What applying a message did to the state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Applied {
    /// A value of this group was stored (tracked ones are now `Confirmed`).
    Value(Group),
    /// Untracked information changed (model, capabilities, ...).
    Info,
    /// Nothing changed.
    Ignored,
}

impl DeviceState {
    /// Stores a decoded message. `Message::Unknown` is left to the caller.
    pub(crate) fn apply(&mut self, table: Table, msg: Message) -> Applied {
        match msg {
            Message::ProtocolInfo { .. } | Message::Unknown { .. } => Applied::Ignored,
            Message::Dsee { on } => {
                self.dsee.confirm(on);
                Applied::Value(Group::Dsee)
            }
            Message::DseeAutoOff { auto } => {
                self.dsee_auto_off.confirm(auto);
                Applied::Value(Group::DseeAutoOff)
            }
            Message::ConnectionMode { value } => {
                self.connection_mode.confirm(value);
                Applied::Value(Group::ConnectionMode)
            }
            Message::WearPause { on } => {
                self.wear_pause.confirm(on);
                Applied::Value(Group::WearPause)
            }
            Message::VoiceAssistant { value } => {
                self.voice_assistant.confirm(value);
                Applied::Value(Group::VoiceAssistant)
            }
            Message::QuickAccess { functions } => {
                self.quick_access.confirm(functions);
                Applied::Value(Group::QuickAccess)
            }
            Message::VoiceGuideVolume { value } => {
                self.voice_guide_volume.confirm(value);
                Applied::Value(Group::VoiceGuide)
            }
            Message::Playback { status } => {
                self.playback = Some(status);
                Applied::Value(Group::Playback)
            }
            Message::SupportFunctions(list) => {
                let ids = list.into_iter().map(|f| f.id).collect();
                match table {
                    Table::T1 => self.capabilities_t1 = ids,
                    Table::T2 => self.capabilities_t2 = ids,
                }
                Applied::Info
            }
            Message::DeviceInfo(info) => {
                match info {
                    DeviceInfo::ModelName(m) => self.model = Some(m),
                    DeviceInfo::Firmware(f) => self.firmware = Some(f),
                    DeviceInfo::SeriesAndColor { color, .. } => self.color = Some(color),
                }
                Applied::Info
            }
            Message::EqUltCapability(c) => {
                self.eq_capability = Some(c);
                Applied::Info
            }
            Message::GsCapability(c) => {
                match self
                    .gs
                    .iter_mut()
                    .find(|g| g.capability.setting == c.setting)
                {
                    Some(g) => g.capability = c,
                    None => self.gs.push(GsSetting {
                        capability: c,
                        current: None,
                    }),
                }
                Applied::Info
            }
            Message::Battery { level, charging } => {
                self.battery.confirm((level, charging));
                Applied::Value(Group::Battery)
            }
            Message::NcAsm(v) => {
                self.nc.confirm(v);
                Applied::Value(Group::NcAsm)
            }
            Message::EqUlt(v) => {
                self.eq_ult.confirm(v);
                Applied::Value(Group::EqUlt)
            }
            Message::Codec(v) => {
                self.codec.confirm(v);
                Applied::Value(Group::Codec)
            }
            Message::Volume(v) => {
                self.volume.confirm(v);
                Applied::Value(Group::Volume)
            }
            Message::AutoPowerOff { current, .. } => {
                self.auto_power_off.confirm(current);
                Applied::Value(Group::AutoPowerOff)
            }
            Message::GsBool {
                setting: GS_MULTIPOINT,
                on,
            } => {
                self.multipoint.confirm(on);
                Applied::Value(Group::GeneralSetting(GS_MULTIPOINT))
            }
            Message::GsBool { setting, on } => {
                match setting {
                    GS_TOUCH_PANEL => self.touch_panel.confirm(on),
                    GS_SIDETONE => self.sidetone.confirm(on),
                    _ => return self.set_gs(setting, GsValue::Bool(on)),
                }
                self.set_gs(setting, GsValue::Bool(on));
                Applied::Value(Group::GeneralSetting(setting))
            }
            Message::GsList { setting, index } => self.set_gs(setting, GsValue::Index(index)),
        }
    }

    fn set_gs(&mut self, setting: u8, value: GsValue) -> Applied {
        match self.gs.iter_mut().find(|g| g.capability.setting == setting) {
            Some(g) => {
                g.current = Some(value);
                Applied::Value(Group::GeneralSetting(setting))
            }
            None => Applied::Ignored,
        }
    }

    /// The status slot for a group, if the group is tracked.
    pub(crate) fn status_mut(&mut self, group: Group) -> Option<&mut Status> {
        Some(match group {
            Group::Battery => &mut self.battery.status,
            Group::NcAsm => &mut self.nc.status,
            Group::EqUlt => &mut self.eq_ult.status,
            Group::Codec => &mut self.codec.status,
            Group::Volume => &mut self.volume.status,
            Group::GeneralSetting(GS_MULTIPOINT) => &mut self.multipoint.status,
            Group::GeneralSetting(GS_TOUCH_PANEL) => &mut self.touch_panel.status,
            Group::GeneralSetting(GS_SIDETONE) => &mut self.sidetone.status,
            Group::AutoPowerOff => &mut self.auto_power_off.status,
            Group::Dsee => &mut self.dsee.status,
            Group::DseeAutoOff => &mut self.dsee_auto_off.status,
            Group::ConnectionMode => &mut self.connection_mode.status,
            Group::WearPause => &mut self.wear_pause.status,
            Group::VoiceAssistant => &mut self.voice_assistant.status,
            Group::QuickAccess => &mut self.quick_access.status,
            Group::VoiceGuide => &mut self.voice_guide_volume.status,
            Group::GeneralSetting(_)
            | Group::Connect
            | Group::Power
            | Group::Raw
            | Group::Playback => return None,
        })
    }

    /// Marks every tracked value unavailable and the device disconnected.
    pub(crate) fn mark_disconnected(&mut self) {
        self.connected = false;
        self.address = None;
        self.channel = None;
        self.playback = None;
        for status in [
            &mut self.battery.status,
            &mut self.nc.status,
            &mut self.eq_ult.status,
            &mut self.codec.status,
            &mut self.volume.status,
            &mut self.multipoint.status,
            &mut self.auto_power_off.status,
            &mut self.dsee.status,
            &mut self.dsee_auto_off.status,
            &mut self.connection_mode.status,
            &mut self.wear_pause.status,
            &mut self.voice_assistant.status,
            &mut self.quick_access.status,
            &mut self.voice_guide_volume.status,
            &mut self.touch_panel.status,
            &mut self.sidetone.status,
        ] {
            *status = Status::Unavailable;
        }
    }
}
