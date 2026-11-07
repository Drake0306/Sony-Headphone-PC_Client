//! Platform dispatch for finding and connecting to devices.

use std::io;

use crate::{Candidate, Transport};

/// Paired devices that are connected and named like the headphones, or whose
/// cached SDP records expose the v2 service.
pub fn discover() -> io::Result<Vec<Candidate>> {
    #[cfg(target_os = "macos")]
    return crate::macos::discover();
    #[cfg(windows)]
    return crate::windows::discover();
    #[cfg(target_os = "linux")]
    return crate::linux::discover();
    #[cfg(not(any(target_os = "macos", windows, target_os = "linux")))]
    return Err(unsupported());
}

/// Opens the v2 RFCOMM channel to the paired device with this address (any
/// notation). Fails with `NotConnected` if the device has no connection to this
/// computer; on macOS a missing v2 record triggers an SDP query and a wait of
/// up to six seconds. On Windows the stack resolves the channel from SDP
/// during the socket connect. On Linux this is
/// [`connect_with_hint`] without a recorded channel.
///
/// On macOS the IOBluetooth calls are dispatched onto the main queue and the
/// received data arrives on the main run loop, so call this and use the
/// transport from a worker thread (the app uses its broker thread); blocking
/// the main thread in `recv_timeout` would starve the delegate.
pub fn connect(address: &str) -> io::Result<Box<dyn Transport>> {
    connect_with_hint(address, None)
}

/// [`connect`] with the RFCOMM channel recorded after an earlier connect.
/// Only Linux uses the hint: BlueZ has no SDP browsing over D-Bus, so it tries
/// `HAIRLINE_RFCOMM_CHANNEL`, then this channel (both used without a probe;
/// the session's handshake proves them), then scans channels 1..=30
/// with the protocol-info GET alone. macOS and Windows resolve the channel
/// from SDP and ignore the hint. `Transport::channel` reports the channel used.
pub fn connect_with_hint(address: &str, channel: Option<u8>) -> io::Result<Box<dyn Transport>> {
    #[cfg(target_os = "macos")]
    {
        let _ = channel;
        crate::macos::connect(address)
    }
    #[cfg(windows)]
    {
        let _ = channel;
        crate::windows::connect(address)
    }
    #[cfg(target_os = "linux")]
    return crate::linux::connect_with_hint(address, channel);
    #[cfg(not(any(target_os = "macos", windows, target_os = "linux")))]
    {
        let _ = (address, channel);
        Err(unsupported())
    }
}

#[cfg(not(any(target_os = "macos", windows, target_os = "linux")))]
fn unsupported() -> io::Error {
    io::Error::new(
        io::ErrorKind::Unsupported,
        "Bluetooth transport is implemented on macOS, Windows and Linux",
    )
}
