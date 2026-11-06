use mdr_frame::{ack_for, encode, Decoder, Frame, FrameType};

#[test]
fn encodes_protocol_info_request() {
    let f = Frame {
        frame_type: FrameType::Command1,
        seq: 0,
        payload: vec![0x00, 0x00],
    };
    assert_eq!(
        encode(&f),
        [0x3E, 0x0C, 0x00, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x0E, 0x3C]
    );
}

#[test]
fn encodes_ack_with_flipped_seq() {
    assert_eq!(
        encode(&ack_for(0)),
        [0x3E, 0x01, 0x01, 0x00, 0x00, 0x00, 0x00, 0x02, 0x3C]
    );
    assert_eq!(
        encode(&ack_for(1)),
        [0x3E, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x3C]
    );
}

#[test]
fn escapes_payload_bytes() {
    let f = Frame {
        frame_type: FrameType::Command1,
        seq: 1,
        payload: vec![0x3E, 0x3C, 0x3D],
    };
    let bytes = encode(&f);
    // payload 3E 3C 3D -> 3D 2E, 3D 2C, 3D 2D ; checksum = 0C+01+03+3E+3C+3D = 0xC7
    assert_eq!(
        bytes,
        [
            0x3E, 0x0C, 0x01, 0x00, 0x00, 0x00, 0x03, 0x3D, 0x2E, 0x3D, 0x2C, 0x3D, 0x2D, 0xC7,
            0x3C
        ]
    );
    let mut d = Decoder::new();
    let out = d.push(&bytes);
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].as_ref().unwrap().payload, vec![0x3E, 0x3C, 0x3D]);
}

#[test]
fn escapes_checksum_byte() {
    // choose payload so checksum == 0x3E: 0C + 00 + 01 + x = 0x3E -> x = 0x31
    let f = Frame {
        frame_type: FrameType::Command1,
        seq: 0,
        payload: vec![0x31],
    };
    let bytes = encode(&f);
    assert_eq!(&bytes[bytes.len() - 3..], [0x3D, 0x2E, 0x3C]);
    let mut d = Decoder::new();
    assert_eq!(d.push(&bytes)[0].as_ref().unwrap().payload, vec![0x31]);
}

#[test]
fn decodes_battery_reply() {
    // 23 00 43 00 from the device: type 0C seq 01 len 4 ; checksum = 0C+01+04+23+00+43+00 = 0x77
    let bytes = [
        0x3E, 0x0C, 0x01, 0x00, 0x00, 0x00, 0x04, 0x23, 0x00, 0x43, 0x00, 0x77, 0x3C,
    ];
    let mut d = Decoder::new();
    let out = d.push(&bytes);
    let f = out[0].as_ref().unwrap();
    assert_eq!(f.frame_type, FrameType::Command1);
    assert_eq!(f.seq, 1);
    assert_eq!(f.payload, vec![0x23, 0x00, 0x43, 0x00]);
}

#[test]
fn decodes_frame_split_across_pushes() {
    let bytes = [
        0x3E, 0x0C, 0x01, 0x00, 0x00, 0x00, 0x04, 0x23, 0x00, 0x43, 0x00, 0x77, 0x3C,
    ];
    let mut d = Decoder::new();
    assert!(d.push(&bytes[..6]).is_empty());
    let out = d.push(&bytes[6..]);
    assert_eq!(out.len(), 1);
    assert_eq!(
        out[0].as_ref().unwrap().payload,
        vec![0x23, 0x00, 0x43, 0x00]
    );
}

#[test]
fn reports_bad_checksum_and_resyncs() {
    let mut bad = vec![
        0x3E, 0x0C, 0x01, 0x00, 0x00, 0x00, 0x04, 0x23, 0x00, 0x43, 0x00, 0x00, 0x3C,
    ];
    bad.extend_from_slice(&[0x3E, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x3C]);
    let mut d = Decoder::new();
    let out = d.push(&bad);
    assert_eq!(out.len(), 2);
    assert!(matches!(
        out[0],
        Err(mdr_frame::FrameError::BadChecksum {
            expected: 0x77,
            got: 0x00
        })
    ));
    assert_eq!(out[1].as_ref().unwrap().frame_type, FrameType::Ack);
}
