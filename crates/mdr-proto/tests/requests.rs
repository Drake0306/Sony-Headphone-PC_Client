use mdr_proto::*;

#[test]
fn builders_produce_documented_bytes() {
    assert_eq!(request::protocol_info().bytes, [0x00, 0x00]);
    assert_eq!(request::support_functions(Table::T2).table, Table::T2);
    assert_eq!(request::battery_get().bytes, [0x22, 0x00]);
    assert_eq!(request::ncasm_get().bytes, [0x66, 0x17]);
    assert_eq!(request::ncasm_set(&NcAsmState { mode: NcMode::Ambient, level: 14, voice_focus: true }).bytes, [0x68, 0x17, 0x01, 0x01, 0x01, 0x01, 0x0E]);
    assert_eq!(request::ncasm_set(&NcAsmState { mode: NcMode::NoiseCancelling, level: 14, voice_focus: true }).bytes, [0x68, 0x17, 0x01, 0x01, 0x00, 0x01, 0x0E]);
    assert_eq!(request::ncasm_set(&NcAsmState { mode: NcMode::Off, level: 14, voice_focus: false }).bytes, [0x68, 0x17, 0x01, 0x00, 0x00, 0x00, 0x0E]);
    assert_eq!(request::eq_ult_set(&EqUltState { preset: 0, ult: UltLevel::Ult1, bands: vec![10; 6] }).bytes, [0x58, 0x03, 0x00, 0x01, 0x06, 0x0A, 0x0A, 0x0A, 0x0A, 0x0A, 0x0A]);
    assert_eq!(request::volume_set(12).bytes, [0xA8, 0x20, 0x0C]);
    assert_eq!(request::gs_set_bool(0xD1, true).bytes, [0xD8, 0xD1, 0x00, 0x00]);
    assert_eq!(request::power_off().bytes, [0x24, 0x03, 0x01]);
    assert_eq!(request::auto_power_off_set(4).bytes, [0x28, 0x05, 0x04, 0x04]);
    assert_eq!(request::device_info(1).bytes, [0x04, 0x01]);
}

#[test]
fn requests_declare_their_capability_and_group() {
    assert_eq!(request::ncasm_set(&NcAsmState { mode: NcMode::Off, level: 1, voice_focus: false }).capability, Some(0x6B));
    assert_eq!(request::eq_ult_get().capability, Some(0x53));
    assert_eq!(request::battery_get().capability, Some(0x20));
    assert_eq!(request::protocol_info().capability, None);
    assert_eq!(request::eq_ult_get().group, Group::EqUlt);
}

#[test]
fn deny_list_is_keyed_on_opcode_and_sub_id() {
    assert!(denied(Table::T1, &[0xF8, 0x09, 0x01]).is_some());
    assert!(denied(Table::T1, &[0xF8, 0x01, 0x01]).is_none());
    assert!(denied(Table::T1, &[0x98, 0x00, 0x16, 0x01]).is_some());
    assert!(denied(Table::T1, &[0x98, 0x00, 0x06, 0x01]).is_none());
    assert!(denied(Table::T1, &[0xC4, 0x00]).is_some());
    assert!(denied(Table::T1, &[0x3E, 0x00]).is_some());
    assert!(denied(Table::T1, &[0x24, 0x03, 0x01]).is_some()); // power off needs session-level confirmation
    assert!(denied(Table::T1, &[0x68, 0x17, 0x01, 0x01, 0x01, 0x00, 0x05]).is_none());
}
