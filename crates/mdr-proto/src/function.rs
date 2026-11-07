//! Support-function names and opcode roles.
//!
//! Sources: the support-function lists and libmdr `FunctionType` names in
//! `docs/research/notes/Sony headphone PC client comparison/ult_wear_support.md`
//! (decoded against libmdr `ProtocolV2.hpp`) and `docs/protocol-notes.md`
//! ("Support function bytes"); the opcode roles follow the T1/T2 opcode
//! tables in `docs/protocol-notes.md`, which were read from libmdr
//! `ProtocolV2T1Enum.hpp` / `ProtocolV2T2.hpp`.

use crate::types::Table;

/// What an opcode does, per the libmdr command tables.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Get,
    Ret,
    Set,
    Notify,
    /// Not in the tables we have; treat it as a SET when asking for confirmation.
    Unknown,
}

/// A readable name for a support-function id, or `None` when unknown.
pub fn name(table: Table, id: u8) -> Option<&'static str> {
    match table {
        Table::T1 => t1_name(id),
        Table::T2 => t2_name(id),
    }
}

/// T1 `FunctionType` names.
fn t1_name(id: u8) -> Option<&'static str> {
    Some(match id {
        0x10 => "Concierge data",
        0x12 => "Codec indicator",
        0x13 => "Upscaling indicator",
        0x14 => "BLE setup",
        0x15 => "Tutorial contents on concierge",
        0x20 => "Battery level indicator",
        0x23 => "Power off",
        0x25 => "Auto power off with wearing detection",
        0x27 => "Tandem keep-alive",
        0x32 => "Firmware update (MTK, without disconnection)",
        0x42 => "Classic only / LE classic setting",
        0x53 => "Preset EQ and ULT mode",
        0x69 => "Ambient sound control mode select",
        0x6B => "Noise cancelling / ambient dual level adjustment",
        0x70 => "Auto noise cancelling / ambient",
        0x90 => "Fixed message",
        0x93 => "Voice assistant alert notification",
        0xA1 => "Playback controller with call volume adjustment",
        0xB2 => "GATT connectable",
        0xB7 => "Head tracker (accelerometer type)",
        0xC1 => "Action log notifier",
        0xC2 => "Time-series operation log notifier",
        0xD1 => "General setting 1",
        0xD2 => "General setting 2",
        0xD3 => "General setting 3",
        0xE2 => "Upscaling auto off",
        0xE3 => "Connection mode (sound quality / LDAC status)",
        0xF1 => "Playback control by wearing",
        0xF4 => "Voice assistant settings",
        0xF9 => "Reset settings",
        0xFD => "Quick access",
        _ => return None,
    })
}

/// T2 `FunctionType` names.
fn t2_name(id: u8) -> Option<&'static str> {
    Some(match id {
        0x42 => "Voice guidance setting (MTK transfer)",
        0x50 => "Safe listening (HBS 1)",
        0xF2 => "Quick access easy setting",
        _ => return None,
    })
}

/// The role of an opcode (the first payload byte) in `table`.
pub fn opcode_role(table: Table, opcode: u8) -> Role {
    match table {
        Table::T1 => t1_role(opcode),
        Table::T2 => t2_role(opcode),
    }
}

/// The T1 table in `docs/protocol-notes.md`, plus `98` ALERT_SET_PARAM.
/// Groups we have no table for (LEA, SENSE, OPT, UPDT, ...) stay `Unknown`.
fn t1_role(op: u8) -> Role {
    match op {
        0x00 | 0x02 | 0x04 | 0x06 | 0x10 | 0x12 | 0x20 | 0x22 | 0x26 | 0x50 | 0x56 | 0x5A
        | 0x60 | 0x66 | 0xA0 | 0xA2 | 0xA6 | 0xD0 | 0xD2 | 0xD6 | 0xE0 | 0xE2 | 0xE6 | 0xF0
        | 0xF2 | 0xF6 | 0xFA => Role::Get,
        0x01 | 0x03 | 0x05 | 0x07 | 0x11 | 0x13 | 0x21 | 0x23 | 0x27 | 0x51 | 0x57 | 0x5B
        | 0x61 | 0x67 | 0xA1 | 0xA3 | 0xA7 | 0xD1 | 0xD3 | 0xD7 | 0xE1 | 0xE3 | 0xE7 | 0xF1
        | 0xF3 | 0xF7 | 0xFB => Role::Ret,
        0x18 | 0x24 | 0x28 | 0x58 | 0x68 | 0x98 | 0xA4 | 0xA8 | 0xC4 | 0xD8 | 0xE8 | 0xF4
        | 0xF8 | 0xFC => Role::Set,
        0x15 | 0x19 | 0x25 | 0x29 | 0x59 | 0x69 | 0xA5 | 0xA9 | 0xC9 | 0xD5 | 0xD9 | 0xE5
        | 0xE9 | 0xF5 | 0xF9 | 0xFD => Role::Notify,
        _ => Role::Unknown,
    }
}

/// T2: `06/07` support functions, and the peripheral (`30..`), voice
/// guidance (`40..`) and safe listening (`50..`) groups, whose low nibble
/// follows the libmdr group layout (`0` get capability, `1` ret, `2` get
/// status, `3` ret, `4` set status, `5` notify, `6` get param, `7` ret,
/// `8` set param, `9` notify). Their extended opcodes (`xA..xD`) and every
/// other group stay `Unknown`.
fn t2_role(op: u8) -> Role {
    match op {
        0x06 => Role::Get,
        0x07 => Role::Ret,
        0x30..=0x5F => match op & 0x0F {
            0x0 | 0x2 | 0x6 => Role::Get,
            0x1 | 0x3 | 0x7 => Role::Ret,
            0x4 | 0x8 => Role::Set,
            0x5 | 0x9 => Role::Notify,
            _ => Role::Unknown,
        },
        _ => Role::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The ULT WEAR's declared lists (fw 2.1.0), from the notes.
    const T1_DECLARED: [u8; 30] = [
        0x10, 0x12, 0x13, 0x14, 0x15, 0x20, 0x23, 0x25, 0x27, 0x32, 0x53, 0x69, 0x6B, 0x70, 0x93,
        0xF4, 0x90, 0xA1, 0xF1, 0xB2, 0xB7, 0xC1, 0xC2, 0xD1, 0xD2, 0xD3, 0xE3, 0xE2, 0xF9, 0xFD,
    ];
    const T2_DECLARED: [u8; 3] = [0x42, 0x50, 0xF2];

    #[test]
    fn function_names_cover_the_declared_set() {
        for id in T1_DECLARED {
            assert!(name(Table::T1, id).is_some(), "T1 {id:#04X} has no name");
        }
        for id in T2_DECLARED {
            assert!(name(Table::T2, id).is_some(), "T2 {id:#04X} has no name");
        }
        assert_eq!(name(Table::T1, 0x12), Some("Codec indicator"));
        assert_eq!(name(Table::T2, 0x50), Some("Safe listening (HBS 1)"));
        assert_eq!(name(Table::T1, 0x01), None);
        assert_eq!(name(Table::T2, 0x12), None);
    }

    #[test]
    fn opcode_roles_follow_the_tables() {
        for op in [
            0x00, 0x06, 0x12, 0x22, 0x56, 0x66, 0xA6, 0xD6, 0xE6, 0xF6, 0xFA,
        ] {
            assert_eq!(opcode_role(Table::T1, op), Role::Get, "T1 {op:#04X}");
        }
        for op in [
            0x18, 0x24, 0x28, 0x58, 0x68, 0x98, 0xA8, 0xC4, 0xD8, 0xE8, 0xF8, 0xFC,
        ] {
            assert_eq!(opcode_role(Table::T1, op), Role::Set, "T1 {op:#04X}");
        }
        assert_eq!(opcode_role(Table::T1, 0x57), Role::Ret);
        assert_eq!(opcode_role(Table::T1, 0x69), Role::Notify);
        assert_eq!(opcode_role(Table::T1, 0x0F), Role::Unknown);
        assert_eq!(opcode_role(Table::T1, 0x30), Role::Unknown);
        assert_eq!(opcode_role(Table::T2, 0x06), Role::Get);
        assert_eq!(opcode_role(Table::T2, 0x46), Role::Get);
        assert_eq!(opcode_role(Table::T2, 0x48), Role::Set);
        assert_eq!(opcode_role(Table::T2, 0x36), Role::Get);
        assert_eq!(opcode_role(Table::T2, 0x3C), Role::Unknown);
        assert_eq!(opcode_role(Table::T2, 0x26), Role::Unknown);
    }
}
