use mdr_proto::*;

#[test]
fn builders_produce_documented_bytes() {
    assert_eq!(request::protocol_info().bytes, [0x00, 0x00]);
    assert_eq!(request::support_functions(Table::T2).table, Table::T2);
    assert_eq!(request::battery_get().bytes, [0x22, 0x00]);
    assert_eq!(request::ncasm_get().bytes, [0x66, 0x17]);
    assert_eq!(
        request::ncasm_set(&NcAsmState {
            mode: NcMode::Ambient,
            level: 14,
            voice_focus: true
        })
        .bytes,
        [0x68, 0x17, 0x01, 0x01, 0x01, 0x01, 0x0E]
    );
    assert_eq!(
        request::ncasm_set(&NcAsmState {
            mode: NcMode::NoiseCancelling,
            level: 14,
            voice_focus: true
        })
        .bytes,
        [0x68, 0x17, 0x01, 0x01, 0x00, 0x01, 0x0E]
    );
    assert_eq!(
        request::ncasm_set(&NcAsmState {
            mode: NcMode::Off,
            level: 14,
            voice_focus: false
        })
        .bytes,
        [0x68, 0x17, 0x01, 0x00, 0x00, 0x00, 0x0E]
    );
    assert_eq!(
        request::eq_ult_set(&EqUltState {
            preset: 0,
            ult: UltLevel::Ult1,
            bands: vec![10; 6]
        })
        .bytes,
        [0x58, 0x03, 0x00, 0x01, 0x06, 0x0A, 0x0A, 0x0A, 0x0A, 0x0A, 0x0A]
    );
    assert_eq!(request::volume_set(12).bytes, [0xA8, 0x20, 0x0C]);
    assert_eq!(
        request::gs_set_bool(0xD1, true).bytes,
        [0xD8, 0xD1, 0x00, 0x00]
    );
    assert_eq!(request::power_off().bytes, [0x24, 0x03, 0x01]);
    assert_eq!(
        request::auto_power_off_set(4).bytes,
        [0x28, 0x05, 0x04, 0x04]
    );
    assert_eq!(request::device_info(1).bytes, [0x04, 0x01]);
}

#[test]
fn requests_declare_their_capability_and_group() {
    assert_eq!(
        request::ncasm_set(&NcAsmState {
            mode: NcMode::Off,
            level: 1,
            voice_focus: false
        })
        .capability,
        Some(0x6B)
    );
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
    assert!(denied(Table::T1, &[0x98, 0x00, 0x07, 0x01]).is_some());
    assert!(denied(Table::T1, &[0x98]).is_some());
    assert!(denied(Table::T1, &[0x98, 0x01, 0x06, 0x01]).is_some());
    assert!(denied(Table::T1, &[0x98, 0x00, 0x06, 0x00]).is_some());
    assert!(denied(Table::T1, &[0x98, 0x00, 0x06, 0x01, 0xFF]).is_some());
    assert!(denied(Table::T1, &[0xC4, 0x00]).is_some());
    assert!(denied(Table::T1, &[0x3E, 0x00]).is_some());
    assert!(denied(Table::T1, &[0x24, 0x03, 0x01]).is_some()); // power off needs session-level confirmation
    assert!(denied(Table::T1, &[0x68, 0x17, 0x01, 0x01, 0x01, 0x00, 0x05]).is_none());
}

#[test]
fn phase2_builders_bytes() {
    use request::*;
    let check = |r: Request, t: Table, b: &[u8], c: Option<u8>, g: Group| {
        assert_eq!(
            (r.table, r.bytes.as_slice(), r.capability, r.group),
            (t, b, c, g)
        );
    };
    let t1 = Table::T1;
    check(
        dsee_status_get(),
        t1,
        &[0x12, 0x03],
        Some(0x13),
        Group::Dsee,
    );
    check(
        dsee_auto_off_get(),
        t1,
        &[0xE6, 0x01],
        Some(0xE2),
        Group::DseeAutoOff,
    );
    check(
        dsee_auto_off_set(true),
        t1,
        &[0xE8, 0x01, 1],
        Some(0xE2),
        Group::DseeAutoOff,
    );
    check(
        connection_mode_get(),
        t1,
        &[0xE6, 0x02],
        Some(0xE3),
        Group::ConnectionMode,
    );
    check(
        connection_mode_set(1),
        t1,
        &[0xE8, 0x02, 1],
        Some(0xE3),
        Group::ConnectionMode,
    );
    check(
        wear_pause_get(),
        t1,
        &[0xF6, 0x01],
        Some(0xF1),
        Group::WearPause,
    );
    check(
        wear_pause_set(true),
        t1,
        &[0xF8, 0x01, 0],
        Some(0xF1),
        Group::WearPause,
    );
    check(
        wear_pause_set(false),
        t1,
        &[0xF8, 0x01, 1],
        Some(0xF1),
        Group::WearPause,
    );
    check(
        voice_assistant_get(),
        t1,
        &[0xF6, 0x04],
        Some(0xF4),
        Group::VoiceAssistant,
    );
    check(
        voice_assistant_set(0x31),
        t1,
        &[0xF8, 0x04, 0x31],
        Some(0xF4),
        Group::VoiceAssistant,
    );
    check(
        quick_access_get(),
        t1,
        &[0xF6, 0x0D],
        Some(0xFD),
        Group::QuickAccess,
    );
    check(
        quick_access_set(&[1, 2]),
        t1,
        &[0xF8, 0x0D, 2, 1, 2],
        Some(0xFD),
        Group::QuickAccess,
    );
    check(
        voice_guide_volume_get(),
        Table::T2,
        &[0x46, 0x20],
        Some(0x42),
        Group::VoiceGuide,
    );
    check(
        voice_guide_volume_set(-2),
        Table::T2,
        &[0x48, 0x20, 0xFE],
        Some(0x42),
        Group::VoiceGuide,
    );
    check(
        playback_status_get(),
        t1,
        &[0xA2, 0x01],
        Some(0xA1),
        Group::Playback,
    );
    let m = multipoint_commit();
    check(
        m.clone(),
        t1,
        &[0x98, 0x00, 0x06, 0x01],
        Some(0xD1),
        Group::GeneralSetting(0xD1),
    );
    assert_eq!(denied(m.table, &m.bytes), None);
}
