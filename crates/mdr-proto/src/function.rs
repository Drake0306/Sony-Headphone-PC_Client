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

/// Whether `id` is a libmdr `FunctionType` value for `table`
/// (`ProtocolV2.hpp`, `t1::FunctionType` / `t2::FunctionType`).
pub fn is_function_type(table: Table, id: u8) -> bool {
    match table {
        Table::T1 => matches!(
            id,
            0x10..=0x1B
                | 0x20..=0x2F
                | 0x30
                | 0x32
                | 0x34..=0x38
                | 0x40..=0x4F
                | 0x50..=0x59
                | 0x61..=0x6D
                | 0x70..=0x75
                | 0x80..=0x84
                | 0x90..=0x94
                | 0xA1..=0xA4
                | 0xB0..=0xB3
                | 0xB5..=0xB8
                | 0xC1..=0xC3
                | 0xD1..=0xD4
                | 0xE1..=0xED
                | 0xF0..=0xFF
        ),
        Table::T2 => matches!(
            id,
            0x20..=0x24
                | 0x30..=0x34
                | 0x40..=0x48
                | 0x50..=0x55
                | 0x60..=0x64
                | 0x6F
                | 0x70..=0x75
                | 0xF0..=0xFF
        ),
    }
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

/// T2: exactly the opcodes libmdr's `ProtocolV2T2.hpp` `Command` enum
/// defines (branch `v1-compat`); every other byte, including the undefined
/// safe-listening `50..52`, is `Unknown`.
fn t2_role(op: u8) -> Role {
    match op {
        // CONNECT, POWER, PERI, VOICE_GUIDANCE, SAFE_LISTENING, LEA, PARTY, SYSTEM
        0x06 | 0x20 | 0x22 | 0x26 | 0x30 | 0x32 | 0x36 | 0x40 | 0x42 | 0x46 | 0x56 | 0x5A
        | 0x60 | 0x62 | 0x66 | 0x70 | 0x72 | 0x76 | 0xF0 | 0xF2 | 0xF6 | 0xFA => Role::Get,
        0x07 | 0x21 | 0x23 | 0x27 | 0x31 | 0x33 | 0x37 | 0x41 | 0x43 | 0x47 | 0x53 | 0x57
        | 0x5B | 0x61 | 0x63 | 0x67 | 0x71 | 0x73 | 0x77 | 0xF1 | 0xF3 | 0xF7 | 0xFB => Role::Ret,
        0x24 | 0x28 | 0x34 | 0x38 | 0x3C | 0x44 | 0x48 | 0x54 | 0x58 | 0x68 | 0x74 | 0x78
        | 0x7C | 0xF4 | 0xF8 | 0xFC => Role::Set,
        0x25 | 0x29 | 0x35 | 0x39 | 0x3D | 0x45 | 0x55 | 0x59 | 0x65 | 0x69 | 0x75 | 0x79
        | 0xF5 | 0xF9 | 0xFD => Role::Notify,
        _ => Role::Unknown,
    }
}

/// The support function (libmdr `FunctionType`) that a GET of `opcode` with
/// `inquired` reads, or `None` when it reads none we can name.
///
/// Rule: an answered GET proves a function only when its inquired type and a
/// `FunctionType` carry the same libmdr name within the opcode's group (both
/// enums are in `ProtocolV2T1.hpp` / `ProtocolV2T2.hpp`; `FunctionType` is
/// in `ProtocolV2.hpp`). The pairs the session already sends agree (battery
/// `22 00` → `20`, NC/ASM `66 17` → `6B`, volume `A6 20` → `A1`, voice
/// guidance volume T2 `46 20` → `42`, ...). NC/ASM names differ only in
/// wording ("mode switch" = dual, "seamless" = level adjustment). Groups
/// whose enums drift apart (T1 COMMON, EQEBB, AUDIO; T2 POWER, PERI, VOICE
/// GUIDANCE, SYSTEM) are written out pair by pair. Connect reads, inquired
/// types without a same-named function (T1 power `07` link control, NC/ASM
/// `30` and `40`, play `40`, T2 system `06` and `07`) and opcodes that are
/// not GETs prove nothing. Every id returned is a libmdr `FunctionType`
/// (`is_function_type`; tested over the whole probe catalogue).
pub fn for_opcode(table: Table, opcode: u8, inquired: Option<u8>) -> Option<u8> {
    if opcode_role(table, opcode) != Role::Get {
        return None;
    }
    let i = inquired?;
    match table {
        Table::T1 => t1_function(opcode & 0xF0, i),
        Table::T2 => t2_function(opcode & 0xF0, i),
    }
}

fn t1_function(group: u8, i: u8) -> Option<u8> {
    Some(match (group, i) {
        // COMMON: concierge, connection status, codec, upscaling, BLE setup;
        // then connection time, special mode, device info, tandem, display fw.
        (0x10, 0x00..=0x04) => 0x10 + i,
        (0x10, 0x05) => 0x16,
        (0x10, 0x06) => 0x18,
        (0x10, 0x07) => 0x19,
        (0x10, 0x08) => 0x1A,
        (0x10, 0x09) => 0x1B,
        // POWER: battery 20 .. touch panel backlight 2F (07 link control has no match).
        (0x20, 0x00..=0x06 | 0x08..=0x0F) => 0x20 + i,
        // EQEBB.
        (0x50, 0x00..=0x03) => 0x50 + i,
        (0x50, 0x04) => 0x57,
        (0x50, 0x30) => 0x54,
        (0x50, 0x31) => 0x55,
        (0x50, 0x32) => 0x56,
        (0x50, 0x33..=0x36) => 0x59,
        (0x50, 0x40) => 0x58,
        // NCASM.
        (0x60, 0x01) => 0x61,
        (0x60, 0x11) => 0x62,
        (0x60, 0x12) => 0x63,
        (0x60, 0x13) => 0x64,
        (0x60, 0x14) => 0x65,
        (0x60, 0x15) => 0x68,
        (0x60, 0x16) => 0x6A,
        (0x60, 0x17) => 0x6B,
        (0x60, 0x18) => 0x6C,
        (0x60, 0x19) => 0x6D,
        (0x60, 0x21) => 0x66,
        (0x60, 0x22) => 0x67,
        // PLAY: playback controllers; volume belongs to the call-volume one.
        (0xA0, 0x01) => 0xA1,
        (0xA0, 0x02) => 0xA3,
        (0xA0, 0x03) => 0xA4,
        (0xA0, 0x20 | 0x21) => 0xA1,
        (0xA0, 0x30 | 0x31) => 0xA2,
        // GENERAL SETTING: the inquired type is the function id.
        (0xD0, 0xD1..=0xD4) => i,
        // AUDIO.
        (0xE0, 0x00..=0x04) => 0xE1 + i,
        (0xE0, 0x05..=0x0B) => 0xE2 + i,
        // SYSTEM: vibrator F0 .. head gesture FF (training shares FF).
        (0xF0, 0x00..=0x0F) => 0xF0 + i,
        (0xF0, 0x10) => 0xFF,
        _ => return None,
    })
}

fn t2_function(group: u8, i: u8) -> Option<u8> {
    Some(match (group, i) {
        // POWER: auto standby, caring charge, USB submersion.
        (0x20, 0x00) => 0x20,
        (0x20, 0x01) => 0x22,
        (0x20, 0x02) => 0x23,
        // PERI.
        (0x30, 0x00..=0x02) => 0x30 + i,
        (0x30, 0x03) => 0x34,
        // VOICE GUIDANCE.
        (0x40, 0x00) => 0x40,
        (0x40, 0x01) => 0x41,
        (0x40, 0x02) => 0x44,
        (0x40, 0x03) => 0x45,
        (0x40, 0x20) => 0x42,
        (0x40, 0x21) => 0x43,
        (0x40, 0x30..=0x32) => 0x16 + i,
        // SAFE LISTENING.
        (0x50, 0x00..=0x05) => 0x50 + i,
        // LEA.
        (0x60, 0x00..=0x04) => 0x60 + i,
        (0x60, 0xFE) => 0x6F,
        // PARTY.
        (0x70, 0x00..=0x05) => 0x70 + i,
        // SYSTEM, by name (`SystemInquiredType` vs `FunctionType`); the two
        // stop lining up after 05.
        (0xF0, 0x00) => 0xF0, // WEARING_STATUS_CHECKER
        (0xF0, 0x01) => 0xF1, // REPEAT_TAP_TRAINING_MODE
        (0xF0, 0x02) => 0xF2, // QUICK_ACCESS_EASY_SETTING
        (0xF0, 0x03) => 0xF3, // AUTO_VOLUME_OPTIMIZER
        (0xF0, 0x04) => 0xF4, // AUTO_VOLUME_WITH_LIMITATION
        (0xF0, 0x05) => 0xF5, // SONY_VOICE_ASSISTANT(_SETTING_...)
        // 06 SONY_VOICE_ASSISTANT_COMMAND, 07 WEARING_DEVICE_INFORMATION: no function.
        (0xF0, 0x08) => 0xF6,        // WEARING_POSITION(_JUDGMENT_BY_SENSOR)
        (0xF0, 0x09) => 0xF7,        // LINK_AUTO_SWITCH_FOR_SPEAKER
        (0xF0, 0x0A) => 0xF8,        // LINK_AUTO_SWITCH_FOR_HEADSETS
        (0xF0, 0x0B) => 0xF9,        // MIC_ON_OFF_BY_HEADPHONE_OPERATION
        (0xF0, 0x0C) => 0xFA,        // FUNCTION_CHANGE
        (0xF0, 0x0D) => 0xFB,        // USB_BROWSER
        (0xF0, 0x0E) => 0xFC,        // LIGHTING_MODE
        (0xF0, 0x0F) => 0xFD,        // VOICE_ASSISTANT_WITH_SPECIFIC_SETUP_LINK_SUPPORT
        (0xF0, 0x10 | 0x11) => 0xFE, // LIGHTING_DEFAULT_COLOR(_COLOR_TYPE / _CUSTOM_COLOR)
        _ => return None,
    })
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

    /// T1 GET and SET opcodes in the `docs/protocol-notes.md` table (plus `98`).
    const T1_GETS: [u8; 27] = [
        0x00, 0x02, 0x04, 0x06, 0x10, 0x12, 0x20, 0x22, 0x26, 0x50, 0x56, 0x5A, 0x60, 0x66, 0xA0,
        0xA2, 0xA6, 0xD0, 0xD2, 0xD6, 0xE0, 0xE2, 0xE6, 0xF0, 0xF2, 0xF6, 0xFA,
    ];
    const T1_SETS: [u8; 14] = [
        0x18, 0x24, 0x28, 0x58, 0x68, 0x98, 0xA4, 0xA8, 0xC4, 0xD8, 0xE8, 0xF4, 0xF8, 0xFC,
    ];

    #[test]
    fn opcode_roles_follow_the_tables() {
        for op in T1_GETS {
            assert_eq!(opcode_role(Table::T1, op), Role::Get, "T1 {op:#04X}");
        }
        for op in T1_SETS {
            assert_eq!(opcode_role(Table::T1, op), Role::Set, "T1 {op:#04X}");
        }
        assert_eq!(opcode_role(Table::T1, 0x57), Role::Ret);
        assert_eq!(opcode_role(Table::T1, 0x69), Role::Notify);
        assert_eq!(opcode_role(Table::T1, 0x0F), Role::Unknown);
        assert_eq!(opcode_role(Table::T1, 0x30), Role::Unknown);
    }

    #[test]
    fn t2_roles_are_only_what_libmdr_defines() {
        assert_eq!(opcode_role(Table::T2, 0x06), Role::Get);
        assert_eq!(opcode_role(Table::T2, 0x46), Role::Get);
        assert_eq!(opcode_role(Table::T2, 0x48), Role::Set);
        assert_eq!(opcode_role(Table::T2, 0x3C), Role::Set);
        assert_eq!(opcode_role(Table::T2, 0x47), Role::Ret);
        assert_eq!(opcode_role(Table::T2, 0x59), Role::Notify);
        // Not defined by libmdr: unknown, so they need confirmation.
        for op in [0x50, 0x51, 0x52, 0x49, 0x3A, 0x64, 0x12] {
            assert_eq!(opcode_role(Table::T2, op), Role::Unknown, "T2 {op:#04X}");
        }
    }

    #[test]
    fn for_opcode_maps_inquired_types_to_function_types() {
        // The pairs the session already uses (request.rs).
        assert_eq!(for_opcode(Table::T1, 0x22, Some(0x00)), Some(0x20));
        assert_eq!(for_opcode(Table::T1, 0x26, Some(0x05)), Some(0x25));
        assert_eq!(for_opcode(Table::T1, 0x12, Some(0x02)), Some(0x12));
        assert_eq!(for_opcode(Table::T1, 0x12, Some(0x03)), Some(0x13));
        assert_eq!(for_opcode(Table::T1, 0x56, Some(0x03)), Some(0x53));
        assert_eq!(for_opcode(Table::T1, 0x66, Some(0x17)), Some(0x6B));
        assert_eq!(for_opcode(Table::T1, 0xA6, Some(0x20)), Some(0xA1));
        assert_eq!(for_opcode(Table::T1, 0xA2, Some(0x01)), Some(0xA1));
        assert_eq!(for_opcode(Table::T1, 0xD6, Some(0xD2)), Some(0xD2));
        assert_eq!(for_opcode(Table::T1, 0xE6, Some(0x01)), Some(0xE2));
        assert_eq!(for_opcode(Table::T1, 0xE6, Some(0x02)), Some(0xE3));
        assert_eq!(for_opcode(Table::T1, 0xF6, Some(0x01)), Some(0xF1));
        assert_eq!(for_opcode(Table::T1, 0xF6, Some(0x04)), Some(0xF4));
        assert_eq!(for_opcode(Table::T1, 0xFA, Some(0x0D)), Some(0xFD));
        assert_eq!(for_opcode(Table::T2, 0x46, Some(0x20)), Some(0x42));
        // Others by libmdr name.
        assert_eq!(for_opcode(Table::T1, 0x10, Some(0x05)), Some(0x16));
        assert_eq!(for_opcode(Table::T1, 0x50, Some(0x40)), Some(0x58));
        assert_eq!(for_opcode(Table::T1, 0xE2, Some(0x05)), Some(0xE7));
        assert_eq!(for_opcode(Table::T1, 0xF6, Some(0x10)), Some(0xFF));
        assert_eq!(for_opcode(Table::T2, 0x32, Some(0x02)), Some(0x32));
        assert_eq!(for_opcode(Table::T2, 0x32, Some(0x03)), Some(0x34));
        assert_eq!(for_opcode(Table::T2, 0x22, Some(0x01)), Some(0x22));
        assert_eq!(for_opcode(Table::T2, 0x56, Some(0x05)), Some(0x55));
        assert_eq!(for_opcode(Table::T2, 0x60, Some(0xFE)), Some(0x6F));
        assert_eq!(for_opcode(Table::T2, 0xF6, Some(0x11)), Some(0xFE));
        // T2 SYSTEM stops lining up after 05: 06 and 07 have no function.
        assert_eq!(for_opcode(Table::T2, 0xF6, Some(0x05)), Some(0xF5));
        assert_eq!(for_opcode(Table::T2, 0xF6, Some(0x06)), None);
        assert_eq!(for_opcode(Table::T2, 0xF6, Some(0x07)), None);
        assert_eq!(for_opcode(Table::T2, 0xF6, Some(0x08)), Some(0xF6));
        assert_eq!(for_opcode(Table::T2, 0xF2, Some(0x0C)), Some(0xFA));
        assert_eq!(for_opcode(Table::T2, 0xFA, Some(0x0F)), Some(0xFD));
        // Nothing to prove: connect reads, unmatched names, no inquired byte, not a GET.
        assert_eq!(for_opcode(Table::T1, 0x04, Some(0x01)), None);
        assert_eq!(for_opcode(Table::T1, 0x26, Some(0x07)), None);
        assert_eq!(for_opcode(Table::T1, 0x66, Some(0x30)), None);
        assert_eq!(for_opcode(Table::T1, 0x22, None), None);
        assert_eq!(for_opcode(Table::T1, 0x24, Some(0x00)), None);
        assert_eq!(for_opcode(Table::T2, 0x48, Some(0x20)), None);
    }

    #[test]
    fn every_catalogue_read_maps_to_a_libmdr_function_type_or_none() {
        for spec in crate::probe::GET_CATALOGUE {
            for &i in spec.inquired {
                if let Some(id) = for_opcode(spec.table, spec.opcode, Some(i)) {
                    assert!(
                        is_function_type(spec.table, id),
                        "{:?} {:#04X}:{i:#04X} maps to {id:#04X}, not a libmdr FunctionType",
                        spec.table,
                        spec.opcode
                    );
                }
            }
        }
        // The lists themselves: spot checks against ProtocolV2.hpp.
        assert!(is_function_type(Table::T1, 0x1B));
        assert!(!is_function_type(Table::T1, 0x1C));
        assert!(!is_function_type(Table::T1, 0x31));
        assert!(!is_function_type(Table::T1, 0x60));
        assert!(!is_function_type(Table::T1, 0xB4));
        assert!(is_function_type(Table::T2, 0x6F));
        assert!(!is_function_type(Table::T2, 0x65));
        assert!(!is_function_type(Table::T2, 0x25));
        assert!(!is_function_type(Table::T2, 0x10));
    }
}
