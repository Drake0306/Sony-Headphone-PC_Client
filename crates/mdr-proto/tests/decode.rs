use mdr_proto::*;

#[test]
fn protocol_info_v2() {
    let (k, m) = decode(Table::T1, &[0x01, 0x00, 0x03, 0x00, 0x20, 0x02, 0x00, 0x00]);
    assert_eq!(k, Kind::Ret);
    assert_eq!(
        m,
        Message::ProtocolInfo {
            version: 0x0300_2002,
            table1: true,
            table2: true
        }
    );
}

#[test]
fn support_functions_list() {
    let (_, m) = decode(Table::T1, &[0x07, 0x00, 0x02, 0x20, 0x01, 0x53, 0x01]);
    assert_eq!(
        m,
        Message::SupportFunctions(vec![
            SupportFunction {
                id: 0x20,
                priority: 1
            },
            SupportFunction {
                id: 0x53,
                priority: 1
            }
        ])
    );
}

#[test]
fn battery_67_percent() {
    let (k, m) = decode(Table::T1, &[0x23, 0x00, 0x43, 0x00]);
    assert_eq!(k, Kind::Ret);
    assert_eq!(
        m,
        Message::Battery {
            level: 67,
            charging: Charging::NotCharging
        }
    );
    let (k2, _) = decode(Table::T1, &[0x25, 0x00, 0x43, 0x01]);
    assert_eq!(k2, Kind::Ntfy);
}

#[test]
fn ncasm_ambient_voice_level_14() {
    let (_, m) = decode(Table::T1, &[0x67, 0x17, 0x01, 0x01, 0x01, 0x01, 0x0E]);
    assert_eq!(
        m,
        Message::NcAsm(NcAsmState {
            mode: NcMode::Ambient,
            level: 14,
            voice_focus: true
        })
    );
    let (k, m) = decode(Table::T1, &[0x69, 0x17, 0x01, 0x00, 0x00, 0x00, 0x0E]);
    assert_eq!(k, Kind::Ntfy);
    assert_eq!(
        m,
        Message::NcAsm(NcAsmState {
            mode: NcMode::Off,
            level: 14,
            voice_focus: false
        })
    );
}

#[test]
fn eq_ult_reply() {
    let (_, m) = decode(
        Table::T1,
        &[
            0x57, 0x03, 0x00, 0x02, 0x06, 0x0A, 0x0A, 0x0A, 0x0A, 0x0A, 0x0A,
        ],
    );
    assert_eq!(
        m,
        Message::EqUlt(EqUltState {
            preset: 0x00,
            ult: UltLevel::Ult2,
            bands: vec![10; 6]
        })
    );
}

#[test]
fn eq_ult_capability_with_presets() {
    let p = [
        0x51, 0x03, 0x06, 0x15, 0x0A, 0x02, 0x00, 0x03, b'O', b'f', b'f', 0x16, 0x04, b'B', b'a',
        b's', b's',
    ];
    let (_, m) = decode(Table::T1, &p);
    assert_eq!(
        m,
        Message::EqUltCapability(EqUltCapability {
            bands: 6,
            steps: 21,
            ult2_additional_step: 10,
            presets: vec![(0x00, "Off".into()), (0x16, "Bass".into())]
        })
    );
}

#[test]
fn codec_ldac_and_model_name() {
    assert_eq!(
        decode(Table::T1, &[0x13, 0x02, 0x10]).1,
        Message::Codec(Codec::Ldac)
    );
    let name = [
        0x05, 0x01, 0x08, b'U', b'L', b'T', b' ', b'W', b'E', b'A', b'R',
    ];
    assert_eq!(
        decode(Table::T1, &name).1,
        Message::DeviceInfo(DeviceInfo::ModelName("ULT WEAR".into()))
    );
}

#[test]
fn gs_bool_inverted_enum() {
    assert_eq!(
        decode(Table::T1, &[0xD7, 0xD1, 0x00, 0x00]).1,
        Message::GsBool {
            setting: 0xD1,
            on: true
        }
    );
    assert_eq!(
        decode(Table::T1, &[0xD7, 0xD1, 0x00, 0x01]).1,
        Message::GsBool {
            setting: 0xD1,
            on: false
        }
    );
}

#[test]
fn short_and_unknown_payloads_do_not_panic() {
    assert!(matches!(
        decode(Table::T1, &[0x23]).1,
        Message::Unknown { .. }
    ));
    assert!(matches!(
        decode(Table::T2, &[0x47, 0x20, 0xFF]).1,
        Message::Unknown { .. }
    ));
    assert!(matches!(decode(Table::T1, &[]).1, Message::Unknown { .. }));
}

#[test]
fn gs_capability_list_with_two_elements() {
    let mut p = vec![0xD1, 0xD2, 0x01, 0x00, 0x05];
    p.extend_from_slice(b"TOUCH");
    p.extend_from_slice(&[0x00, 0x02, 0x00, 0x02]);
    p.extend_from_slice(b"ON");
    p.extend_from_slice(&[0x00, 0x00, 0x03]);
    p.extend_from_slice(b"OFF");
    p.push(0x00);
    assert_eq!(
        decode(Table::T1, &p).1,
        Message::GsCapability(GsCapability {
            setting: 0xD2,
            kind: GsKind::List,
            subject: "TOUCH".into(),
            summary: "".into(),
            elements: vec![("ON".into(), "".into()), ("OFF".into(), "".into())],
        })
    );
}

#[test]
fn t2_support_functions_list() {
    let (k, m) = decode(
        Table::T2,
        &[0x07, 0x00, 0x03, 0x42, 0x01, 0x50, 0x01, 0xF2, 0x01],
    );
    assert_eq!(k, Kind::Ret);
    assert_eq!(
        m,
        Message::SupportFunctions(vec![
            SupportFunction {
                id: 0x42,
                priority: 1
            },
            SupportFunction {
                id: 0x50,
                priority: 1
            },
            SupportFunction {
                id: 0xF2,
                priority: 1
            },
        ])
    );
}
