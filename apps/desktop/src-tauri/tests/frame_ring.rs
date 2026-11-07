//! The frame ring: the last N frames, oldest first.

use hairline_lib::frame_ring::{FrameDir, FrameEntry, FrameRing};
use mdr_proto::Table;

fn entry(t_ms: u64) -> FrameEntry {
    FrameEntry {
        t_ms,
        dir: FrameDir::Sent,
        table: Some(Table::T1),
        bytes: vec![0x3E, 0x0C, 0x00, 0x3C],
    }
}

#[test]
fn frame_ring_keeps_the_last_500() {
    let mut ring = FrameRing::new(500);
    assert!(ring.snapshot().is_empty());
    for t in 0..600 {
        ring.push(entry(t));
    }
    let snap = ring.snapshot();
    assert_eq!(snap.len(), 500);
    assert_eq!(snap.first().unwrap().t_ms, 100);
    assert_eq!(snap.last().unwrap().t_ms, 599);
}

#[test]
fn received_chunks_are_split_into_frames() {
    let mut ring = FrameRing::new(10);
    // One ACK and the start of a T1 frame, then the rest of it.
    ring.push_received(
        5,
        &[
            0x3E, 0x01, 0x01, 0x00, 0x00, 0x00, 0x00, 0x02, 0x3C, 0x3E, 0x0C,
        ],
    );
    ring.push_received(6, &[0x00, 0x00, 0x00, 0x00, 0x01, 0x22, 0x2F, 0x3C]);
    let snap = ring.snapshot();
    assert_eq!(snap.len(), 2, "{snap:?}");
    assert_eq!(snap[0].dir, FrameDir::Received);
    assert_eq!(snap[0].table, None);
    assert_eq!(snap[0].t_ms, 5);
    assert_eq!(snap[1].table, Some(Table::T1));
    assert_eq!(snap[1].t_ms, 6);
    assert_eq!(
        snap[1].bytes,
        [0x3E, 0x0C, 0x00, 0x00, 0x00, 0x00, 0x01, 0x22, 0x2F, 0x3C]
    );
}

#[test]
fn sent_frames_carry_their_table() {
    let mut ring = FrameRing::new(10);
    ring.push_sent(1, &[0x3E, 0x0E, 0x00, 0x3C]);
    let snap = ring.snapshot();
    assert_eq!(snap[0].dir, FrameDir::Sent);
    assert_eq!(snap[0].table, Some(Table::T2));
}
