//! Device state as the UI sees it, and how decoded messages update it.

use mdr_proto::{
    Charging, Codec, DeviceInfo, EqUltCapability, EqUltState, Group, GsCapability, Message,
    NcAsmState, Table,
};
use serde::{Deserialize, Serialize};

/// General setting that drives `DeviceState::multipoint`.
pub const GS_MULTIPOINT: u8 = 0xD1;

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
    pub gs: Vec<GsSetting>,
    pub devices: Vec<PairedDevice>,
}

/// What applying a message did to the state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Applied {
    /// A tracked value of this group was stored and is now `Confirmed`.
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
            Message::GsBool { setting, on } => self.set_gs(setting, GsValue::Bool(on)),
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
            Group::AutoPowerOff => &mut self.auto_power_off.status,
            Group::GeneralSetting(_) | Group::Connect | Group::Power | Group::Raw => return None,
        })
    }

    /// Marks every tracked value unavailable and the device disconnected.
    pub(crate) fn mark_disconnected(&mut self) {
        self.connected = false;
        for status in [
            &mut self.battery.status,
            &mut self.nc.status,
            &mut self.eq_ult.status,
            &mut self.codec.status,
            &mut self.volume.status,
            &mut self.multipoint.status,
            &mut self.auto_power_off.status,
        ] {
            *status = Status::Unavailable;
        }
    }
}
