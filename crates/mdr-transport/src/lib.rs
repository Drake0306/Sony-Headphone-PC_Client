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

/// Whether a Bluetooth device name looks like the ULT WEAR headphones.
pub fn is_ult_wear_name(name: &str) -> bool {
    name.contains("ULT WEAR") || name.contains("Sony ULT")
}

/// A Bluetooth address reduced to lowercase hex digits, so `AA:BB:..`,
/// `aa-bb-..` and any mix of case compare equal.
pub fn normalise_address(address: &str) -> String {
    address
        .chars()
        .filter(char::is_ascii_hexdigit)
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

/// A paired device that is connected and looks like the headphones, or that
/// exposes the v2 service.
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
    /// RFCOMM channel the link runs on, when the transport knows it.
    fn channel(&self) -> Option<u8> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn addresses_compare_in_any_notation() {
        assert_eq!(normalise_address("A8:E6:E8:FE:D6:54"), "a8e6e8fed654");
        assert_eq!(
            normalise_address("a8-e6-e8-fe-d6-54"),
            normalise_address("A8:e6:E8:fe:D6:54")
        );
    }

    #[test]
    fn ult_wear_names_match() {
        assert!(is_ult_wear_name("ULT WEAR"));
        assert!(is_ult_wear_name("Sony ULT WEAR (Roy)"));
        assert!(is_ult_wear_name("Sony ULT"));
        assert!(!is_ult_wear_name("WH-1000XM5"));
    }
}
