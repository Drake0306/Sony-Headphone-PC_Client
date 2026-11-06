use crate::opcode::{inquired, t1};
use crate::types::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Ret,
    Ntfy,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Message {
    ProtocolInfo { version: u32, table1: bool, table2: bool },
    SupportFunctions(Vec<SupportFunction>),
    DeviceInfo(DeviceInfo),
    Battery { level: u8, charging: Charging },
    NcAsm(NcAsmState),
    EqUlt(EqUltState),
    EqUltCapability(EqUltCapability),
    Codec(Codec),
    Volume(u8),
    GsCapability(GsCapability),
    GsBool { setting: u8, on: bool },
    GsList { setting: u8, index: u8 },
    AutoPowerOff { current: u8, last: u8 },
    Unknown { table: Table, opcode: u8, payload: Vec<u8> },
}

/// Byte cursor that returns `None` instead of panicking on short input.
struct Reader<'a>(&'a [u8]);

impl<'a> Reader<'a> {
    fn u8(&mut self) -> Option<u8> {
        let (&b, rest) = self.0.split_first()?;
        self.0 = rest;
        Some(b)
    }

    fn take(&mut self, n: usize) -> Option<&'a [u8]> {
        if self.0.len() < n {
            return None;
        }
        let (head, rest) = self.0.split_at(n);
        self.0 = rest;
        Some(head)
    }

    /// `u8 length + bytes`, lossy UTF-8.
    fn string(&mut self) -> Option<String> {
        let n = usize::from(self.u8()?);
        Some(String::from_utf8_lossy(self.take(n)?).into_owned())
    }
}

fn kind_of(opcode: u8) -> Kind {
    match opcode & 0x0F {
        0x1 | 0x3 | 0x7 | 0xB => Kind::Ret,
        0x5 | 0x9 | 0xD => Kind::Ntfy,
        _ => Kind::Other,
    }
}

/// Decode a frame payload (bytes after the frame header). Never panics.
pub fn decode(table: Table, payload: &[u8]) -> (Kind, Message) {
    let Some(&opcode) = payload.first() else {
        return (Kind::Other, unknown(table, 0, payload));
    };
    let kind = kind_of(opcode);
    let msg = match table {
        Table::T1 => decode_t1(opcode, payload),
        Table::T2 => None,
    };
    (kind, msg.unwrap_or_else(|| unknown(table, opcode, payload)))
}

fn unknown(table: Table, opcode: u8, payload: &[u8]) -> Message {
    Message::Unknown { table, opcode, payload: payload.to_vec() }
}

fn decode_t1(opcode: u8, payload: &[u8]) -> Option<Message> {
    let mut r = Reader(&payload[1..]);
    let sub = r.u8()?;
    match opcode {
        t1::CONNECT_RET_PROTOCOL_INFO => {
            let v = r.take(4)?;
            let version = u32::from_be_bytes([v[0], v[1], v[2], v[3]]);
            // libmdr EnableDisable: ENABLE = 0x00, DISABLE = 0x01, so 0x00 means enabled.
            Some(Message::ProtocolInfo { version, table1: r.u8()? == 0, table2: r.u8()? == 0 })
        }
        t1::CONNECT_RET_SUPPORT_FUNCTION => {
            let n = r.u8()?;
            let mut list = Vec::new();
            for _ in 0..n {
                list.push(SupportFunction { id: r.u8()?, priority: r.u8()? });
            }
            Some(Message::SupportFunctions(list))
        }
        t1::CONNECT_RET_DEVICE_INFO => match sub {
            0x01 => Some(Message::DeviceInfo(DeviceInfo::ModelName(r.string()?))),
            0x02 => Some(Message::DeviceInfo(DeviceInfo::Firmware(r.string()?))),
            0x03 => Some(Message::DeviceInfo(DeviceInfo::SeriesAndColor { series: r.u8()?, color: r.u8()? })),
            _ => None,
        },
        t1::POWER_RET_STATUS | t1::POWER_NTFY_STATUS if sub == inquired::BATTERY_SINGLE => {
            let level = r.u8()?;
            let charging = match r.u8()? {
                0 => Charging::NotCharging,
                1 => Charging::Charging,
                3 => Charging::Charged,
                _ => Charging::Unknown,
            };
            Some(Message::Battery { level, charging })
        }
        t1::POWER_RET_PARAM | t1::POWER_NTFY_PARAM if sub == inquired::AUTO_POWER_OFF => {
            Some(Message::AutoPowerOff { current: r.u8()?, last: r.u8()? })
        }
        t1::NCASM_RET_PARAM | t1::NCASM_NTFY_PARAM if sub == inquired::NC_ASM_DUAL_NC_SWITCH_AND_ASM_SEAMLESS => {
            let _value_change = r.u8()?;
            let total = r.u8()?;
            let ambient = r.u8()? == 1;
            let voice_focus = r.u8()? == 1;
            let level = r.u8()?;
            let mode = match (total, ambient) {
                (0, _) => NcMode::Off,
                (_, false) => NcMode::NoiseCancelling,
                (_, true) => NcMode::Ambient,
            };
            Some(Message::NcAsm(NcAsmState { mode, level, voice_focus }))
        }
        t1::EQEBB_RET_PARAM | t1::EQEBB_NTFY_PARAM if sub == inquired::PRESET_EQ_AND_ULT_MODE => {
            let preset = r.u8()?;
            let ult = match r.u8()? {
                0 => UltLevel::Off,
                1 => UltLevel::Ult1,
                2 => UltLevel::Ult2,
                _ => return None,
            };
            let n = usize::from(r.u8()?);
            Some(Message::EqUlt(EqUltState { preset, ult, bands: r.take(n)?.to_vec() }))
        }
        t1::EQEBB_RET_CAPABILITY if sub == inquired::PRESET_EQ_AND_ULT_MODE => {
            let (bands, steps, ult2_additional_step) = (r.u8()?, r.u8()?, r.u8()?);
            let n = r.u8()?;
            let mut presets = Vec::new();
            for _ in 0..n {
                presets.push((r.u8()?, r.string()?));
            }
            Some(Message::EqUltCapability(EqUltCapability { bands, steps, ult2_additional_step, presets }))
        }
        t1::COMMON_RET_STATUS | t1::COMMON_NTFY_STATUS if sub == inquired::CODEC => {
            Some(Message::Codec(match r.u8()? {
                0x00 => Codec::Unsettled,
                0x01 => Codec::Sbc,
                0x02 => Codec::Aac,
                0x10 => Codec::Ldac,
                0x30 => Codec::Lc3,
                other => Codec::Other(other),
            }))
        }
        t1::PLAY_RET_PARAM | t1::PLAY_NTFY_PARAM if sub == inquired::VOLUME => Some(Message::Volume(r.u8()?)),
        t1::GS_RET_CAPABILITY => {
            let kind = match r.u8()? {
                inquired::GS_BOOLEAN => GsKind::Boolean,
                inquired::GS_LIST => GsKind::List,
                _ => return None,
            };
            let _string_format = r.u8()?;
            let subject = r.string()?;
            let summary = r.string()?;
            let mut elements = Vec::new();
            if kind == GsKind::List {
                for _ in 0..r.u8()? {
                    let _element_string_format = r.u8()?;
                    elements.push((r.string()?, r.string()?));
                }
            }
            Some(Message::GsCapability(GsCapability { setting: sub, kind, subject, summary, elements }))
        }
        t1::GS_RET_PARAM | t1::GS_NTFY_PARAM => {
            let kind = r.u8()?;
            let value = r.u8()?;
            match kind {
                // Inverted enum: 0x00 is ON.
                inquired::GS_BOOLEAN => Some(Message::GsBool { setting: sub, on: value == 0 }),
                inquired::GS_LIST => Some(Message::GsList { setting: sub, index: value }),
                _ => None,
            }
        }
        _ => None,
    }
}
