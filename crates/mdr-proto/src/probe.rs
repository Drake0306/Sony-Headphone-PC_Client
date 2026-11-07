//! The opcode probe's catalogue: every read-only GET the probe may send.
//!
//! Each entry is a GET opcode with the inquired types libmdr defines for its
//! group. Sources (libmdr, branch `v1-compat`): `ProtocolV2T1.hpp` `enum class
//! Command` and the `*InquiredType` / `DeviceInfoType` enums (T1),
//! `ProtocolV2T2.hpp` `enum class Command` and its `*InquiredType` enums (T2).
//! The GET structs there are all `{ command, inquired }`.
//!
//! Left out on purpose:
//! - `00` / `06` (protocol info, support functions): the handshake already
//!   sent them at connect.
//! - T1 SYSTEM inquired `09` RESET_SETTINGS: nothing near a reset is sent,
//!   not even its read (owner ruling: no reset anywhere in the app).
//! - T2 LEA inquired `03` GET_IDENTITY_RESOLVING_KEY: its reply is a pairing
//!   secret and would land in the capture file.
//! - Every `OUT_OF_RANGE` (`FF`) value.
//!
//! `probe_points` re-checks each entry against the libmdr role table and the
//! deny-list, so a wrong edit here cannot make the probe send a SET.

use crate::function::{opcode_role, Role};
use crate::types::Table;

/// One GET opcode and the inquired types to try with it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProbeSpec {
    pub table: Table,
    pub opcode: u8,
    /// One request per byte; empty means the opcode is sent alone.
    pub inquired: &'static [u8],
    /// The libmdr command name.
    pub name: &'static str,
}

/// One request the probe sends: `[opcode, inquired]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProbePoint {
    pub table: Table,
    pub opcode: u8,
    pub inquired: Option<u8>,
    pub name: &'static str,
}

impl ProbePoint {
    pub fn bytes(&self) -> Vec<u8> {
        let mut b = vec![self.opcode];
        b.extend(self.inquired);
        b
    }
}

// T1 inquired types (ProtocolV2T1.hpp).
/// `ConnectInquiredType::FIXED_VALUE`.
const T1_CONNECT: &[u8] = &[0x00];
/// `DeviceInfoType`: model name, firmware version, series and colour, guide.
const T1_DEVICE_INFO: &[u8] = &[0x01, 0x02, 0x03, 0x04];
/// `CommonInquiredType` 00..09.
const T1_COMMON: &[u8] = &[0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09];
/// `PowerInquiredType` 00..0F.
const T1_POWER: &[u8] = &[
    0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F,
];
/// `EqEbbInquiredType`.
const T1_EQEBB: &[u8] = &[
    0x00, 0x01, 0x02, 0x03, 0x04, 0x30, 0x31, 0x32, 0x33, 0x34, 0x35, 0x36, 0x40,
];
/// `NcAsmInquiredType`.
const T1_NCASM: &[u8] = &[
    0x01, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x21, 0x22, 0x30, 0x40,
];
/// `PlayInquiredType`.
const T1_PLAY: &[u8] = &[0x01, 0x02, 0x03, 0x20, 0x21, 0x30, 0x31, 0x40];
/// `GsInquiredType`: general settings 1..4.
const T1_GS: &[u8] = &[0xD1, 0xD2, 0xD3, 0xD4];
/// `AudioInquiredType` 00..0B.
const T1_AUDIO: &[u8] = &[
    0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B,
];
/// `SystemInquiredType` 00..10 without `09` RESET_SETTINGS.
const T1_SYSTEM: &[u8] = &[
    0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F, 0x10,
];

// T2 inquired types (ProtocolV2T2.hpp).
/// `PowerInquiredType`.
const T2_POWER: &[u8] = &[0x00, 0x01, 0x02];
/// `PeripheralInquiredType`.
const T2_PERI: &[u8] = &[0x00, 0x01, 0x02, 0x03];
/// `VoiceGuidanceInquiredType`.
const T2_VOICE_GUIDANCE: &[u8] = &[0x00, 0x01, 0x02, 0x03, 0x20, 0x21, 0x30, 0x31, 0x32];
/// `SafeListeningInquiredType`.
const T2_SAFE_LISTENING: &[u8] = &[0x00, 0x01, 0x02, 0x03, 0x04, 0x05];
/// `LEAInquiredType` without `03` GET_IDENTITY_RESOLVING_KEY.
const T2_LEA: &[u8] = &[0x00, 0x01, 0x02, 0x04, 0xFE];
/// `PartyInquiredType`.
const T2_PARTY: &[u8] = &[0x00, 0x01, 0x02, 0x03, 0x04, 0x05];
/// `SystemInquiredType` 00..11.
const T2_SYSTEM: &[u8] = &[
    0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F,
    0x10, 0x11,
];

const fn t1(opcode: u8, inquired: &'static [u8], name: &'static str) -> ProbeSpec {
    ProbeSpec {
        table: Table::T1,
        opcode,
        inquired,
        name,
    }
}

const fn t2(opcode: u8, inquired: &'static [u8], name: &'static str) -> ProbeSpec {
    ProbeSpec {
        table: Table::T2,
        opcode,
        inquired,
        name,
    }
}

/// Every GET the probe sends, T1 then T2, in opcode order.
pub const GET_CATALOGUE: &[ProbeSpec] = &[
    t1(0x02, T1_CONNECT, "CONNECT_GET_CAPABILITY_INFO"),
    t1(0x04, T1_DEVICE_INFO, "CONNECT_GET_DEVICE_INFO"),
    t1(0x10, T1_COMMON, "COMMON_GET_CAPABILITY"),
    t1(0x12, T1_COMMON, "COMMON_GET_STATUS"),
    t1(0x20, T1_POWER, "POWER_GET_CAPABILITY"),
    t1(0x22, T1_POWER, "POWER_GET_STATUS"),
    t1(0x26, T1_POWER, "POWER_GET_PARAM"),
    t1(0x50, T1_EQEBB, "EQEBB_GET_CAPABILITY"),
    t1(0x56, T1_EQEBB, "EQEBB_GET_PARAM"),
    t1(0x5A, T1_EQEBB, "EQEBB_GET_EXTENDED_INFO"),
    t1(0x60, T1_NCASM, "NCASM_GET_CAPABILITY"),
    t1(0x66, T1_NCASM, "NCASM_GET_PARAM"),
    t1(0xA0, T1_PLAY, "PLAY_GET_CAPABILITY"),
    t1(0xA2, T1_PLAY, "PLAY_GET_STATUS"),
    t1(0xA6, T1_PLAY, "PLAY_GET_PARAM"),
    t1(0xD0, T1_GS, "GENERAL_SETTING_GET_CAPABILITY"),
    t1(0xD2, T1_GS, "GENERAL_SETTING_GET_STATUS"),
    t1(0xD6, T1_GS, "GENERAL_SETTING_GET_PARAM"),
    t1(0xE0, T1_AUDIO, "AUDIO_GET_CAPABILITY"),
    t1(0xE2, T1_AUDIO, "AUDIO_GET_STATUS"),
    t1(0xE6, T1_AUDIO, "AUDIO_GET_PARAM"),
    t1(0xF0, T1_SYSTEM, "SYSTEM_GET_CAPABILITY"),
    t1(0xF2, T1_SYSTEM, "SYSTEM_GET_STATUS"),
    t1(0xF6, T1_SYSTEM, "SYSTEM_GET_PARAM"),
    t1(0xFA, T1_SYSTEM, "SYSTEM_GET_EXT_PARAM"),
    t2(0x20, T2_POWER, "POWER_GET_CAPABILITY"),
    t2(0x22, T2_POWER, "POWER_GET_STATUS"),
    t2(0x26, T2_POWER, "POWER_GET_PARAM"),
    t2(0x30, T2_PERI, "PERI_GET_CAPABILITY"),
    t2(0x32, T2_PERI, "PERI_GET_STATUS"),
    t2(0x36, T2_PERI, "PERI_GET_PARAM"),
    t2(0x40, T2_VOICE_GUIDANCE, "VOICE_GUIDANCE_GET_CAPABILITY"),
    t2(0x42, T2_VOICE_GUIDANCE, "VOICE_GUIDANCE_GET_STATUS"),
    t2(0x46, T2_VOICE_GUIDANCE, "VOICE_GUIDANCE_GET_PARAM"),
    t2(0x56, T2_SAFE_LISTENING, "SAFE_LISTENING_GET_PARAM"),
    t2(0x5A, T2_SAFE_LISTENING, "SAFE_LISTENING_GET_EXTENDED_PARAM"),
    t2(0x60, T2_LEA, "LEA_GET_CAPABILITY"),
    t2(0x62, T2_LEA, "LEA_GET_STATUS"),
    t2(0x66, T2_LEA, "LEA_GET_PARAM"),
    t2(0x70, T2_PARTY, "PARTY_GET_CAPABILITY"),
    t2(0x72, T2_PARTY, "PARTY_GET_STATUS"),
    t2(0x76, T2_PARTY, "PARTY_GET_PARAM"),
    t2(0xF0, T2_SYSTEM, "SYSTEM_GET_CAPABILITY"),
    t2(0xF2, T2_SYSTEM, "SYSTEM_GET_STATUS"),
    t2(0xF6, T2_SYSTEM, "SYSTEM_GET_PARAM"),
    t2(0xFA, T2_SYSTEM, "SYSTEM_GET_EXTENDED_PARAM"),
];

/// Whether `opcode` is a GET in the libmdr role table for `table`.
pub fn is_get(table: Table, opcode: u8) -> bool {
    opcode_role(table, opcode) == Role::Get
}

/// Whether the probe may send `bytes`: a GET opcode and not deny-listed.
pub fn may_send(table: Table, bytes: &[u8]) -> bool {
    bytes.first().is_some_and(|&op| is_get(table, op)) && crate::denied(table, bytes).is_none()
}

/// The catalogue expanded into single requests, keeping only those
/// `may_send` allows.
pub fn probe_points(catalogue: &[ProbeSpec]) -> Vec<ProbePoint> {
    let mut points = Vec::new();
    for spec in catalogue {
        let point = |inquired| ProbePoint {
            table: spec.table,
            opcode: spec.opcode,
            inquired,
            name: spec.name,
        };
        if spec.inquired.is_empty() {
            points.push(point(None));
        }
        points.extend(spec.inquired.iter().map(|&i| point(Some(i))));
    }
    points.retain(|p| may_send(p.table, &p.bytes()));
    points
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The libmdr GET lists, written out independently of `function.rs`.
    const T1_GETS: &[u8] = &[
        0x00, 0x02, 0x04, 0x06, 0x10, 0x12, 0x20, 0x22, 0x26, 0x50, 0x56, 0x5A, 0x60, 0x66, 0xA0,
        0xA2, 0xA6, 0xD0, 0xD2, 0xD6, 0xE0, 0xE2, 0xE6, 0xF0, 0xF2, 0xF6, 0xFA,
    ];
    const T2_GETS: &[u8] = &[
        0x06, 0x20, 0x22, 0x26, 0x30, 0x32, 0x36, 0x40, 0x42, 0x46, 0x56, 0x5A, 0x60, 0x62, 0x66,
        0x70, 0x72, 0x76, 0xF0, 0xF2, 0xF6, 0xFA,
    ];

    #[test]
    fn probe_catalogue_is_get_only() {
        for spec in GET_CATALOGUE {
            let list = match spec.table {
                Table::T1 => T1_GETS,
                Table::T2 => T2_GETS,
            };
            assert!(
                list.contains(&spec.opcode),
                "{:?} {:#04X} is not a libmdr GET",
                spec.table,
                spec.opcode
            );
            assert!(is_get(spec.table, spec.opcode), "{spec:?}");
            assert_eq!(opcode_role(spec.table, spec.opcode), Role::Get, "{spec:?}");
        }
        // The handshake GETs are left out.
        assert!(!GET_CATALOGUE.iter().any(|s| matches!(
            (s.table, s.opcode),
            (Table::T1, 0x00 | 0x06) | (Table::T2, 0x06)
        )));
        // Every request the probe builds starts with a GET.
        for p in probe_points(GET_CATALOGUE) {
            assert!(is_get(p.table, p.bytes()[0]), "{p:?}");
        }
    }

    #[test]
    fn probe_catalogue_has_no_denied_bytes() {
        for spec in GET_CATALOGUE {
            for &i in spec.inquired {
                assert_eq!(
                    crate::denied(spec.table, &[spec.opcode, i]),
                    None,
                    "{spec:?} {i:#04X}"
                );
            }
            if spec.inquired.is_empty() {
                assert_eq!(crate::denied(spec.table, &[spec.opcode]), None);
            }
        }
        // Nothing is filtered out: the catalogue itself is clean.
        let total: usize = GET_CATALOGUE.iter().map(|s| s.inquired.len().max(1)).sum();
        assert_eq!(probe_points(GET_CATALOGUE).len(), total);
    }

    #[test]
    fn probe_points_drop_a_wrongly_edited_entry() {
        let bad = [
            t1(0x24, &[0x00], "POWER_SET_STATUS"),
            t1(0xF8, &[0x09], "SYSTEM_SET_PARAM"),
            t1(0x98, &[], "ALERT_SET_PARAM"),
            t2(0x48, &[0x20], "VOICE_GUIDANCE_SET_PARAM"),
            t1(0x22, &[0x00], "POWER_GET_STATUS"),
        ];
        let points = probe_points(&bad);
        assert_eq!(points.len(), 1);
        assert_eq!(points[0].bytes(), vec![0x22, 0x00]);
    }

    #[test]
    fn probe_catalogue_skips_reset_and_secrets() {
        for p in probe_points(GET_CATALOGUE) {
            let system = matches!(p.opcode, 0xF0 | 0xF2 | 0xF6 | 0xFA);
            assert!(
                !(p.table == Table::T1 && system && p.inquired == Some(0x09)),
                "{p:?}"
            );
            assert!(
                !(p.table == Table::T2
                    && (0x60..=0x66).contains(&p.opcode)
                    && p.inquired == Some(0x03)),
                "{p:?}"
            );
            assert_ne!(p.inquired, Some(0xFF), "{p:?}");
        }
    }

    #[test]
    fn probe_catalogue_covers_what_answered_on_the_owners_unit() {
        let points = probe_points(GET_CATALOGUE);
        let has = |table, bytes: &[u8]| {
            points
                .iter()
                .any(|p| p.table == table && p.bytes() == bytes)
        };
        // docs/hardware-validation.md §3.
        for b in [[0x32, 0x02], [0x46, 0x20]] {
            assert!(has(Table::T2, &b), "T2 {b:02X?}");
        }
        for b in [
            [0xA2, 0x01],
            [0x12, 0x03],
            [0xE6, 0x02],
            [0xF6, 0x01],
            [0xF6, 0x04],
            [0xF6, 0x0D],
        ] {
            assert!(has(Table::T1, &b), "T1 {b:02X?}");
        }
    }
}
