//! Byte transports to the headphones: the `Transport` trait, a scripted fake
//! device for tests, and the platform RFCOMM implementation.

mod discover;
mod fake;
#[cfg(target_os = "macos")]
mod macos;

use std::io;
use std::time::Duration;

pub use discover::{connect, discover};
pub use fake::FakeDevice;

/// SDP service UUID of the Sony MDR v2 protocol.
pub const V2_SERVICE_UUID: &str = "956C7B26-D49A-4BA8-B03F-B17D393CB6E2";

/// A paired device that exposes the v2 service.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    pub name: String,
    pub address: String,
    /// Whether the device currently has a baseband (audio) connection.
    pub connected_audio: bool,
}

/// A bidirectional byte link to the device.
pub trait Transport: Send {
    fn write(&mut self, bytes: &[u8]) -> io::Result<()>;
    /// Next chunk of received bytes; `Ok(None)` on timeout, `Err` on link loss.
    fn recv_timeout(&mut self, timeout: Duration) -> io::Result<Option<Vec<u8>>>;
    fn close(&mut self);
}
