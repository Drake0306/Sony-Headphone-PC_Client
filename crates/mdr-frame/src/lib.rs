//! Framing layer: raw bytes <-> frames.

mod codec;
mod decoder;

pub use codec::{ack_for, encode, Frame, FrameError, FrameType};
pub use decoder::Decoder;
