//! Platform dispatch for finding and connecting to devices.

use std::io;

use crate::{Candidate, Transport};

/// Paired devices that are connected and named like the headphones, or whose
/// cached SDP records expose the v2 service.
pub fn discover() -> io::Result<Vec<Candidate>> {
    #[cfg(target_os = "macos")]
    return crate::macos::discover();
    #[cfg(not(target_os = "macos"))]
    return Err(unsupported());
}

/// Opens the v2 RFCOMM channel to the paired device with this address (any
/// notation). Fails with `NotConnected` if the device has no connection to this
/// computer; on macOS a missing v2 record triggers an SDP query and a wait of
/// up to six seconds.
///
/// On macOS the IOBluetooth calls are dispatched onto the main queue and the
/// received data arrives on the main run loop, so call this and use the
/// transport from a worker thread (the app uses its broker thread); blocking
/// the main thread in `recv_timeout` would starve the delegate.
pub fn connect(address: &str) -> io::Result<Box<dyn Transport>> {
    #[cfg(target_os = "macos")]
    return crate::macos::connect(address);
    #[cfg(not(target_os = "macos"))]
    {
        let _ = address;
        Err(unsupported())
    }
}

#[cfg(not(target_os = "macos"))]
fn unsupported() -> io::Error {
    io::Error::new(
        io::ErrorKind::Unsupported,
        "Bluetooth transport is only implemented on macOS in this phase",
    )
}
