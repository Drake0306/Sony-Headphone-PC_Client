mod common;

use common::*;
use mdr_frame::Frame;
use mdr_frame::FrameType;
use mdr_proto::{Charging, Codec, NcAsmState, NcMode, UltLevel};
use mdr_session::*;

#[test]
fn handshake_sends_protocol_info_first_and_waits_for_ack() {
    let mut s = Session::new(0);
    s.start();
    let out = drain(&mut s, 0);
    assert_eq!(out, vec![frame(FrameType::Command1, 0, &[0x00, 0x00])]);
    assert!(s.poll_output(0).is_none(), "one frame in flight");
    s.handle_input(&frame(FrameType::Ack, 1, &[]), 10);
    s.handle_input(
        &frame(
            FrameType::Command1,
            0,
            &[0x01, 0x00, 0x03, 0x00, 0x20, 0x02, 0x00, 0x00],
        ),
        11,
    );
    let out = drain(&mut s, 11);
    assert_eq!(
        out[0],
        frame(FrameType::Ack, 1, &[]),
        "device data frames are ACKed immediately"
    );
    assert_eq!(out[1], frame(FrameType::Command1, 1, &[0x06, 0x00]));
}

#[test]
fn full_handshake_reaches_connected_and_reads_initial_state() {
    let mut s = Session::new(0);
    s.start();
    let script: Vec<(&[u8], &[u8])> = vec![
        // (expected request, device reply) in order
        (
            &[0x00, 0x00],
            &[0x01, 0x00, 0x03, 0x00, 0x20, 0x02, 0x00, 0x00],
        ),
        (
            &[0x06, 0x00],
            &[
                0x07, 0x00, 0x05, 0x20, 0x01, 0x53, 0x01, 0x6B, 0x01, 0x12, 0x01, 0xD1, 0x01,
            ],
        ),
        // T2 support functions
        (&[0x06, 0x00], &[0x07, 0x00, 0x01, 0x42, 0x01]),
        (
            &[0x04, 0x01],
            &[
                0x05, 0x01, 0x08, b'U', b'L', b'T', b' ', b'W', b'E', b'A', b'R',
            ],
        ),
        (
            &[0x04, 0x02],
            &[0x05, 0x02, 0x05, b'2', b'.', b'1', b'.', b'0'],
        ),
        (&[0x04, 0x03], &[0x05, 0x03, 0x00, 0x08]),
        (&[0x22, 0x00], &[0x23, 0x00, 0x43, 0x00]),
        (&[0x66, 0x17], &[0x67, 0x17, 0x01, 0x01, 0x01, 0x01, 0x0E]),
        (&[0x50, 0x03], &[0x51, 0x03, 0x06, 0x15, 0x0A, 0x00]),
        (
            &[0x56, 0x03],
            &[
                0x57, 0x03, 0x00, 0x01, 0x06, 0x0A, 0x0A, 0x0A, 0x0A, 0x0A, 0x0A,
            ],
        ),
        (&[0x12, 0x02], &[0x13, 0x02, 0x10]),
        (
            &[0xD0, 0xD1],
            &[
                0xD1, 0xD1, 0x00, 0x00, 0x0A, b'M', b'U', b'L', b'T', b'I', b'P', b'O', b'I', b'N',
                b'T', 0x00,
            ],
        ),
        (&[0xD6, 0xD1], &[0xD7, 0xD1, 0x00, 0x00]),
        // T2 voice guidance volume (T2 declares 0x42)
        (&[0x46, 0x20], &[0x47, 0x20, 0x02]),
    ];
    let mut events = vec![];
    let mut t = 0;
    for (i, (expect, reply)) in script.iter().enumerate() {
        let out = drain(&mut s, t);
        let req = out.last().unwrap();
        assert_eq!(&decode_payload(req), expect, "request #{i}");
        let seq = seq_of(req);
        events.extend(s.handle_input(&frame(FrameType::Ack, 1 - seq, &[]), t));
        let table = if i == 2 || i == script.len() - 1 {
            FrameType::Command2
        } else {
            FrameType::Command1
        };
        events.extend(s.handle_input(&frame(table, seq, reply), t + 1));
        t += 10;
    }
    assert!(events.contains(&Event::Connected));
    let st = s.state();
    assert!(st.connected);
    assert_eq!(st.model.as_deref(), Some("ULT WEAR"));
    assert_eq!(st.firmware.as_deref(), Some("2.1.0"));
    assert_eq!(st.color, Some(0x08));
    assert_eq!(st.capabilities_t1, vec![0x20, 0x53, 0x6B, 0x12, 0xD1]);
    assert_eq!(st.battery.value, Some((67, Charging::NotCharging)));
    assert_eq!(
        st.nc.value,
        Some(NcAsmState {
            mode: NcMode::Ambient,
            level: 14,
            voice_focus: true
        })
    );
    assert_eq!(st.eq_ult.value.as_ref().unwrap().ult, UltLevel::Ult1);
    assert_eq!(st.codec.value, Some(Codec::Ldac));
    assert_eq!(st.multipoint.value, Some(true));
    assert!(matches!(st.battery.status, Status::Confirmed));
}

#[test]
fn ntfy_without_request_updates_state() {
    let mut s = connected_session(); // helper: runs the scripted device
                                     // The handshake's last device reply used seq 0, so the device's next frame is seq 1.
    let ev = s.handle_input(
        &frame(
            FrameType::Command1,
            1,
            &[0x69, 0x17, 0x01, 0x01, 0x00, 0x00, 0x0E],
        ),
        500,
    );
    assert!(ev.contains(&Event::StateChanged));
    assert_eq!(s.state().nc.value.unwrap().mode, NcMode::NoiseCancelling);
    assert!(matches!(s.state().nc.status, Status::Confirmed));
    assert_eq!(drain(&mut s, 500)[0], frame(FrameType::Ack, 0, &[]));
}

#[test]
fn set_nc_marks_pending_then_confirmed_on_ntfy() {
    let mut s = connected_session();
    s.set_nc(NcAsmState {
        mode: NcMode::Off,
        level: 14,
        voice_focus: true,
    })
    .unwrap();
    assert!(matches!(s.state().nc.status, Status::Pending { .. }));
    let req = drain(&mut s, 500).pop().unwrap();
    assert_eq!(
        decode_payload(&req),
        [0x68, 0x17, 0x01, 0x00, 0x00, 0x01, 0x0E]
    );
    let seq = seq_of(&req);
    s.handle_input(&frame(FrameType::Ack, 1 - seq, &[]), 600);
    s.handle_input(
        &frame(
            FrameType::Command1,
            seq,
            &[0x69, 0x17, 0x01, 0x00, 0x00, 0x01, 0x0E],
        ),
        601,
    );
    assert!(matches!(s.state().nc.status, Status::Confirmed));
    assert_eq!(s.state().nc.value.unwrap().mode, NcMode::Off);
}

#[test]
fn set_ult_is_read_modify_write() {
    let mut s = connected_session();
    s.set_ult(UltLevel::Ult2).unwrap();
    let req = drain(&mut s, 500).pop().unwrap();
    assert_eq!(
        decode_payload(&req),
        [0x58, 0x03, 0x00, 0x02, 0x06, 0x0A, 0x0A, 0x0A, 0x0A, 0x0A, 0x0A]
    );
}

#[test]
fn set_without_reply_becomes_refused_and_requeries() {
    let mut s = connected_session();
    s.set_ult(UltLevel::Off).unwrap();
    let req = drain(&mut s, 500).pop().unwrap();
    let seq = seq_of(&req);
    s.handle_input(&frame(FrameType::Ack, 1 - seq, &[]), 1000);
    assert_eq!(s.next_timeout_ms(), Some(1000 + REPLY_TIMEOUT_MS));
    let ev = s.handle_timeout(2500);
    assert!(ev.contains(&Event::StateChanged));
    assert!(matches!(&s.state().eq_ult.status, Status::Refused { reason } if reason == "no reply"));
    assert_eq!(
        decode_payload(&drain(&mut s, 2500).pop().unwrap()),
        [0x56, 0x03]
    );
}

#[test]
fn unacked_frame_is_retransmitted_then_gives_up() {
    let mut s = Session::new(0);
    s.start();
    let first = drain(&mut s, 0).pop().unwrap();
    for n in 1..=ACK_RETRIES {
        let ev = s.handle_timeout(n * ACK_TIMEOUT_MS);
        if n < ACK_RETRIES {
            assert_eq!(drain(&mut s, n * ACK_TIMEOUT_MS).pop().unwrap(), first);
        } else {
            assert!(ev.contains(&Event::Disconnected));
        }
    }
}

#[test]
fn disconnect_marks_all_unavailable() {
    let mut s = connected_session();
    let ev = s.mark_disconnected();
    assert!(ev.contains(&Event::Disconnected));
    assert!(!s.state().connected);
    assert!(matches!(s.state().battery.status, Status::Unavailable));
    assert!(matches!(s.state().nc.status, Status::Unavailable));
}

#[test]
fn set_link_round_trip() {
    let mut s = connected_session();
    s.set_link(Some("A8:E6:E8:FE:D6:54".into()), Some(18));
    assert_eq!(s.state().address.as_deref(), Some("A8:E6:E8:FE:D6:54"));
    assert_eq!(s.state().channel, Some(18));
    s.mark_disconnected();
    assert_eq!(s.state().address, None);
    assert_eq!(s.state().channel, None);
}

/// Next data frame the session hands out, draining any ACKs before it.
fn next_request(s: &mut Session, now: u64) -> Option<Frame> {
    drain(s, now)
        .iter()
        .map(|b| decode_one(b))
        .find(|f| f.frame_type != FrameType::Ack)
}

#[test]
fn handshake_read_without_reply_continues() {
    let caps = [0x20, 0x53, 0x6B, 0x12, 0xD1];
    let mut s = Session::new(0);
    s.start();
    let (mut t, mut dev_seq, mut events) = (0, 0, vec![]);
    while let Some(req) = next_request(&mut s, t) {
        events.extend(s.handle_input(&frame(FrameType::Ack, 1 - req.seq, &[]), t));
        if req.payload == [0x66, 0x17] {
            events.extend(s.handle_timeout(t + 1600));
            assert!(
                matches!(&s.state().nc.status, Status::Refused { reason } if reason == "no reply")
            );
            let next = next_request(&mut s, t + 1600).expect("handshake continues");
            assert_eq!(next.payload, [0x50, 0x03]);
            t += 1610;
            events.extend(s.handle_input(&frame(FrameType::Ack, 1 - next.seq, &[]), t));
            let reply = scripted_reply(next.frame_type, &next.payload, &caps).unwrap();
            events.extend(s.handle_input(&frame(next.frame_type, dev_seq, &reply), t + 1));
        } else {
            let reply = scripted_reply(req.frame_type, &req.payload, &caps).unwrap();
            events.extend(s.handle_input(&frame(req.frame_type, dev_seq, &reply), t + 1));
        }
        dev_seq ^= 1;
        t += 10;
    }
    assert!(events.contains(&Event::Connected));
    assert!(s.state().connected);
    assert!(matches!(&s.state().nc.status, Status::Refused { .. }));
}

#[test]
fn request_after_idle_is_not_retransmitted() {
    let mut s = connected_session();
    s.handle_timeout(5000);
    s.set_ult(UltLevel::Off).unwrap();
    assert!(s.poll_output(5000).is_some(), "the SET goes out");
    s.handle_timeout(5500);
    assert!(
        s.poll_output(5500).is_none(),
        "no retransmission 500 ms after sending"
    );
    assert_eq!(s.next_timeout_ms(), Some(6000));
}

#[test]
fn request_is_stamped_with_the_poll_clock() {
    // No timeout call between the handshake and the request: only
    // `poll_output` tells the session the time.
    let mut s = connected_session();
    s.set_ult(UltLevel::Off).unwrap();
    assert!(s.poll_output(5000).is_some(), "the SET goes out");
    s.handle_timeout(5500);
    assert!(
        s.poll_output(5500).is_none(),
        "no retransmission 500 ms after sending"
    );
    assert_eq!(s.next_timeout_ms(), Some(6000));
}

#[test]
fn handshake_reads_phase2_fields() {
    let s = connected_session();
    let st = s.state();
    assert_eq!(st.dsee.value, Some(true));
    assert_eq!(st.dsee_auto_off.value, Some(true));
    assert_eq!(st.connection_mode.value, Some(0));
    assert_eq!(st.wear_pause.value, Some(false));
    assert_eq!(st.voice_assistant.value, Some(0xFF));
    assert_eq!(st.quick_access.value, Some(vec![0, 0]));
    assert_eq!(st.voice_guide_volume.value, Some(2));
    assert_eq!(st.playback, Some(2));
    assert_eq!(st.touch_panel.value, Some(false));
    assert_eq!(st.sidetone.value, Some(true));
    assert_eq!(st.auto_power_off.value, Some(0x10));
    assert_eq!(st.volume.value, Some(28));
    for (name, status) in [
        ("dsee", &st.dsee.status),
        ("dsee_auto_off", &st.dsee_auto_off.status),
        ("connection_mode", &st.connection_mode.status),
        ("wear_pause", &st.wear_pause.status),
        ("voice_assistant", &st.voice_assistant.status),
        ("quick_access", &st.quick_access.status),
        ("voice_guide_volume", &st.voice_guide_volume.status),
        ("touch_panel", &st.touch_panel.status),
        ("sidetone", &st.sidetone.status),
        ("auto_power_off", &st.auto_power_off.status),
        ("multipoint", &st.multipoint.status),
        ("volume", &st.volume.status),
    ] {
        assert_eq!(status, &Status::Confirmed, "{name}");
    }
}

/// Puts the EQ state `A2 / Ult1 / [10,13,10,14,20,12]` into a connected session.
fn session_with_custom_eq() -> Session {
    let mut s = connected_session();
    s.handle_input(
        &frame(
            FrameType::Command1,
            1,
            &[
                0x59, 0x03, 0xA2, 0x01, 0x06, 0x0A, 0x0D, 0x0A, 0x0E, 0x14, 0x0C,
            ],
        ),
        500,
    );
    drain(&mut s, 500); // the ACK for the NTFY
    let eq = s.state().eq_ult.value.clone().unwrap();
    assert_eq!(eq.preset, 0xA2);
    assert_eq!(eq.ult, UltLevel::Ult1);
    s
}

#[test]
fn set_eq_preset_keeps_ult_and_bands() {
    let mut s = session_with_custom_eq();
    s.set_eq_preset(0x16).unwrap();
    let req = next_request(&mut s, 600).unwrap();
    assert_eq!(
        req.payload,
        [0x58, 0x03, 0x16, 0x01, 0x06, 0x0A, 0x0D, 0x0A, 0x0E, 0x14, 0x0C]
    );
}

#[test]
fn set_eq_bands_validates_length() {
    let mut s = session_with_custom_eq();
    assert_eq!(
        s.set_eq_bands(vec![1, 2, 3, 4, 5]),
        Err(SessionError::InvalidValue)
    );
    assert!(s.poll_output(600).is_none(), "nothing sent");
    s.set_eq_bands(vec![1, 2, 3, 4, 5, 6]).unwrap();
    let req = next_request(&mut s, 600).unwrap();
    assert_eq!(
        req.payload,
        [0x58, 0x03, 0xA2, 0x01, 0x06, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06]
    );
}

#[test]
fn set_multipoint_sends_bool_then_commit() {
    let mut s = connected_session();
    s.set_multipoint(false).unwrap();
    let out = drain(&mut s, 500);
    assert_eq!(out.len(), 1, "one frame in flight");
    let first = decode_one(&out[0]);
    assert_eq!(first.payload, [0xD8, 0xD1, 0x00, 0x01]);
    assert!(s.poll_output(510).is_none(), "commit waits for the ACK");
    s.handle_input(&frame(FrameType::Ack, 1 - first.seq, &[]), 520);
    let second = next_request(&mut s, 520).expect("commit after ACK");
    assert_eq!(second.payload, [0x98, 0x00, 0x06, 0x01]);
}

#[test]
fn set_multipoint_on_sends_wire_zero() {
    let mut s = connected_session();
    s.set_multipoint(true).unwrap();
    let first = next_request(&mut s, 500).unwrap();
    assert_eq!(first.payload, [0xD8, 0xD1, 0x00, 0x00]);
}

#[test]
fn set_voice_guide_volume_t2() {
    let mut s = connected_session();
    s.set_voice_guide_volume(-2).unwrap();
    let req = next_request(&mut s, 500).unwrap();
    assert_eq!(req.frame_type, FrameType::Command2);
    assert_eq!(req.payload, [0x48, 0x20, 0xFE]);
    assert_eq!(s.set_voice_guide_volume(3), Err(SessionError::InvalidValue));
}

#[test]
fn set_wear_pause_sends_inverted_enum() {
    let mut s = connected_session();
    s.set_wear_pause(true).unwrap();
    assert!(matches!(
        s.state().wear_pause.status,
        Status::Pending { .. }
    ));
    let req = next_request(&mut s, 500).unwrap();
    assert_eq!(req.payload, [0xF8, 0x01, 0x00]);
}

#[test]
fn invalid_volume_rejected() {
    let mut s = connected_session();
    assert_eq!(s.set_volume(31), Err(SessionError::InvalidValue));
    assert!(s.set_volume(30).is_ok());
}

#[test]
fn invalid_auto_power_off_rejected() {
    let mut s = connected_session();
    assert_eq!(s.set_auto_power_off(0x05), Err(SessionError::InvalidValue));
    s.set_auto_power_off(0x11).unwrap();
    let req = next_request(&mut s, 500).unwrap();
    assert_eq!(req.payload, [0x28, 0x05, 0x11, 0x11]);
}

#[test]
fn touch_panel_and_sidetone_use_general_settings() {
    let mut s = connected_session();
    s.set_touch_panel(true).unwrap();
    let req = next_request(&mut s, 500).unwrap();
    assert_eq!(req.payload, [0xD8, 0xD2, 0x00, 0x00]);
    s.handle_input(&frame(FrameType::Ack, 1 - req.seq, &[]), 510);
    s.handle_input(
        &frame(FrameType::Command1, 1, &[0xD9, 0xD2, 0x00, 0x00]),
        520,
    );
    assert_eq!(s.state().touch_panel.value, Some(true));
    assert_eq!(s.state().touch_panel.status, Status::Confirmed);
    s.set_sidetone(false).unwrap();
    let req = next_request(&mut s, 530).unwrap();
    assert_eq!(req.payload, [0xD8, 0xD3, 0x00, 0x01]);
}

#[test]
fn playback_ntfy_updates_untracked_state() {
    let mut s = connected_session();
    let ev = s.handle_input(
        &frame(FrameType::Command1, 1, &[0xA5, 0x01, 0x00, 0x01, 0x00]),
        500,
    );
    assert!(ev.contains(&Event::StateChanged));
    assert_eq!(s.state().playback, Some(0x01));
}

#[test]
fn t2_support_timeout_still_queues_reads() {
    let caps = [0x20, 0x6B];
    let mut s = Session::new(0);
    s.start();
    let (mut t, mut dev_seq, mut events) = (0, 0, vec![]);
    let mut sent: Vec<(FrameType, Vec<u8>)> = vec![];
    let mut timed_out = false;
    while let Some(req) = next_request(&mut s, t) {
        sent.push((req.frame_type, req.payload.clone()));
        events.extend(s.handle_input(&frame(FrameType::Ack, 1 - req.seq, &[]), t));
        if req.frame_type == FrameType::Command2 && req.payload == [0x06, 0x00] {
            // Never answered: the T2 support-function read times out.
            assert!(s.handle_timeout(t + REPLY_TIMEOUT_MS - 1).is_empty());
            events.extend(s.handle_timeout(t + REPLY_TIMEOUT_MS + 100));
            timed_out = true;
            t += REPLY_TIMEOUT_MS + 110;
            let next = next_request(&mut s, t).expect("handshake continues");
            assert_eq!(next.payload, [0x04, 0x01], "first fixed step follows");
            sent.push((next.frame_type, next.payload.clone()));
            events.extend(s.handle_input(&frame(FrameType::Ack, 1 - next.seq, &[]), t));
            let reply = scripted_reply(next.frame_type, &next.payload, &caps).unwrap();
            events.extend(s.handle_input(&frame(next.frame_type, dev_seq, &reply), t + 1));
        } else {
            let reply = scripted_reply(req.frame_type, &req.payload, &caps)
                .unwrap_or_else(|| panic!("no scripted reply for {:02X?}", req.payload));
            events.extend(s.handle_input(&frame(req.frame_type, dev_seq, &reply), t + 1));
        }
        dev_seq ^= 1;
        t += 10;
    }
    assert!(timed_out, "the T2 support-function read was sent");
    let payloads: Vec<&[u8]> = sent.iter().map(|(_, p)| p.as_slice()).collect();
    assert!(payloads.contains(&&[0x22, 0x00][..]), "battery read sent");
    assert!(payloads.contains(&&[0x66, 0x17][..]), "NC/ASM read sent");
    assert!(
        !sent
            .iter()
            .any(|(ft, p)| *ft == FrameType::Command2 && p == &[0x46, 0x20]),
        "no T2 voice guidance read without a T2 list"
    );
    assert!(events.contains(&Event::Connected));
    assert!(s.state().connected);
}
