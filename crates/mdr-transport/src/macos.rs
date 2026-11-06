//! macOS RFCOMM transport over IOBluetooth.
//!
//! Every IOBluetooth call runs on the main queue, and the channel delegate's
//! callbacks arrive on the main run loop, so the application's main run loop
//! must be running. Use the transport from a worker thread: blocking the main
//! thread in `recv_timeout` would starve the delegate.

use std::ffi::c_void;
use std::io;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::time::Duration;

use dispatch2::DispatchQueue;
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObject, NSObjectProtocol};
use objc2::{define_class, msg_send, AllocAnyThread, DefinedClass, MainThreadMarker};
use objc2_io_bluetooth::{
    BluetoothRFCOMMChannelID, IOBluetoothDevice, IOBluetoothRFCOMMChannel, IOBluetoothRFCOMMChannelDelegate,
    IOBluetoothSDPUUID,
};

use crate::{Candidate, Transport, V2_SERVICE_UUID};

/// Largest single `writeSync` call; larger writes are split.
const WRITE_CHUNK: usize = 512;

define_class!(
    /// Forwards received RFCOMM data into a channel; an empty `Vec` signals closure.
    #[unsafe(super(NSObject))]
    #[name = "MdrTransportRFCOMMDelegate"]
    #[ivars = Sender<Vec<u8>>]
    struct Delegate;

    unsafe impl NSObjectProtocol for Delegate {}

    unsafe impl IOBluetoothRFCOMMChannelDelegate for Delegate {
        #[unsafe(method(rfcommChannelData:data:length:))]
        fn data(&self, _channel: Option<&IOBluetoothRFCOMMChannel>, data: *mut c_void, length: usize) {
            if data.is_null() || length == 0 {
                return;
            }
            // SAFETY: IOBluetooth passes a buffer of `length` bytes valid for this call.
            let bytes = unsafe { std::slice::from_raw_parts(data.cast::<u8>(), length) };
            let _ = self.ivars().send(bytes.to_vec());
        }

        #[unsafe(method(rfcommChannelClosed:))]
        fn closed(&self, _channel: Option<&IOBluetoothRFCOMMChannel>) {
            let _ = self.ivars().send(Vec::new());
        }
    }
);

impl Delegate {
    fn new(tx: Sender<Vec<u8>>) -> Retained<Self> {
        let this = Self::alloc().set_ivars(tx);
        // SAFETY: NSObject's designated initialiser.
        unsafe { msg_send![super(this), init] }
    }
}

/// Holds an Objective-C object that is only used inside main-queue closures.
struct MainOnly<T>(T);
// SAFETY: the wrapped objects are only messaged on the main queue (see `on_main`);
// retain/release are thread-safe.
unsafe impl<T> Send for MainOnly<T> {}
// SAFETY: as above; shared references are only dereferenced on the main queue.
unsafe impl<T> Sync for MainOnly<T> {}

/// Runs `f` on the main queue, directly if already on the main thread.
fn on_main<R: Send>(f: impl FnOnce() -> R + Send) -> R {
    if MainThreadMarker::new().is_some() {
        return f();
    }
    let mut result = None;
    DispatchQueue::main().exec_sync(|| result = Some(f()));
    result.expect("main-queue work ran")
}

fn io_err(what: &str, code: i32) -> io::Error {
    io::Error::other(format!("{what} failed: IOReturn {code:#010x}"))
}

/// The 16 bytes of a textual UUID.
fn uuid_bytes(uuid: &str) -> [u8; 16] {
    let hex: Vec<u8> = uuid.bytes().filter(u8::is_ascii_hexdigit).collect();
    assert_eq!(hex.len(), 32, "UUID must have 32 hex digits");
    let mut out = [0u8; 16];
    for (i, pair) in hex.chunks(2).enumerate() {
        let s = std::str::from_utf8(pair).expect("ASCII hex");
        out[i] = u8::from_str_radix(s, 16).expect("hex digit");
    }
    out
}

fn v2_uuid() -> io::Result<Retained<IOBluetoothSDPUUID>> {
    let bytes = uuid_bytes(V2_SERVICE_UUID);
    // SAFETY: `bytes` is 16 readable bytes.
    unsafe { IOBluetoothSDPUUID::uuidWithBytes_length(bytes.as_ptr().cast(), bytes.len() as u32) }
        .ok_or_else(|| io::Error::other("could not build the v2 service UUID"))
}

/// Paired devices. Must run on the main queue.
fn paired_devices() -> Vec<Retained<IOBluetoothDevice>> {
    // SAFETY: class method without arguments.
    let Some(array) = (unsafe { IOBluetoothDevice::pairedDevices() }) else {
        return Vec::new();
    };
    array.to_vec().into_iter().filter_map(|obj| obj.downcast::<IOBluetoothDevice>().ok()).collect()
}

fn string(s: Option<Retained<objc2_foundation::NSString>>) -> String {
    s.map(|s| s.to_string()).unwrap_or_default()
}

pub fn discover() -> io::Result<Vec<Candidate>> {
    on_main(|| {
        let uuid = v2_uuid()?;
        let candidates = paired_devices()
            .into_iter()
            // SAFETY: plain IOBluetoothDevice queries on the main queue.
            .filter(|device| unsafe { device.getServiceRecordForUUID(Some(&uuid)) }.is_some())
            .map(|device| unsafe {
                Candidate {
                    name: string(device.nameOrAddress()),
                    address: string(device.addressString()),
                    connected_audio: device.isConnected(),
                }
            })
            .collect();
        Ok(candidates)
    })
}

/// Channel and delegate of an open link.
type Link = MainOnly<(Retained<IOBluetoothRFCOMMChannel>, Retained<Delegate>)>;

pub fn connect(address: &str) -> io::Result<Box<dyn Transport>> {
    let (tx, rx) = mpsc::channel();
    let link: Link = on_main(move || {
        let uuid = v2_uuid()?;
        let device = paired_devices()
            .into_iter()
            .find(|d| string(unsafe { d.addressString() }).eq_ignore_ascii_case(address))
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, format!("no paired device {address}")))?;
        // SAFETY: IOBluetooth calls on the main queue with valid arguments.
        let record = unsafe { device.getServiceRecordForUUID(Some(&uuid)) }
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "device does not expose the v2 service"))?;
        let mut channel_id: BluetoothRFCOMMChannelID = 0;
        let ret = unsafe { record.getRFCOMMChannelID(&mut channel_id) };
        if ret != 0 {
            return Err(io_err("getRFCOMMChannelID", ret));
        }
        let delegate = Delegate::new(tx);
        let delegate_obj: &AnyObject = &delegate;
        let mut channel = None;
        let ret = unsafe {
            device.openRFCOMMChannelSync_withChannelID_delegate(Some(&mut channel), channel_id, Some(delegate_obj))
        };
        if ret != 0 {
            if let Some(channel) = channel {
                // SAFETY: the channel holds the delegate unretained; detach it before
                // `delegate` is released at the end of this closure.
                unsafe {
                    channel.setDelegate(None);
                    channel.closeChannel();
                }
            }
            return Err(io_err("openRFCOMMChannelSync", ret));
        }
        let channel = channel.ok_or_else(|| io::Error::other("openRFCOMMChannelSync returned no channel"))?;
        Ok(MainOnly((channel, delegate)))
    })?;
    Ok(Box::new(MacTransport { link: Some(link), rx, closed: false }))
}

struct MacTransport {
    /// `None` once closed.
    link: Option<Link>,
    rx: Receiver<Vec<u8>>,
    closed: bool,
}

fn broken_pipe() -> io::Error {
    io::Error::new(io::ErrorKind::BrokenPipe, "RFCOMM channel closed")
}

impl Transport for MacTransport {
    fn write(&mut self, bytes: &[u8]) -> io::Result<()> {
        let link = self.link.as_ref().filter(|_| !self.closed).ok_or_else(broken_pipe)?;
        on_main(|| {
            let channel = &link.0 .0;
            // SAFETY: state query on the main queue.
            if !unsafe { channel.isOpen() } {
                return Err(broken_pipe());
            }
            for chunk in bytes.chunks(WRITE_CHUNK) {
                let mut buf = chunk.to_vec();
                // SAFETY: `buf` is valid for `len` bytes for the duration of the call; len <= 512.
                let ret = unsafe { channel.writeSync_length(buf.as_mut_ptr().cast(), buf.len() as u16) };
                if ret != 0 {
                    // A write that fails because the remote closed is link loss.
                    if !unsafe { channel.isOpen() } {
                        return Err(broken_pipe());
                    }
                    return Err(io_err("writeSync", ret));
                }
            }
            Ok(())
        })
    }

    fn recv_timeout(&mut self, timeout: Duration) -> io::Result<Option<Vec<u8>>> {
        if self.closed {
            return Err(broken_pipe());
        }
        match self.rx.recv_timeout(timeout) {
            Ok(bytes) if bytes.is_empty() => {
                self.closed = true;
                Err(broken_pipe())
            }
            Ok(bytes) => Ok(Some(bytes)),
            Err(RecvTimeoutError::Timeout) => Ok(None),
            Err(RecvTimeoutError::Disconnected) => {
                self.closed = true;
                Err(broken_pipe())
            }
        }
    }

    fn close(&mut self) {
        self.closed = true;
        if let Some(link) = self.link.take() {
            on_main(move || {
                let (channel, _delegate) = &link.0;
                // SAFETY: on the main queue. The channel holds the delegate unretained,
                // so detach it before the link (and the delegate) is released here.
                unsafe {
                    channel.setDelegate(None);
                    channel.closeChannel();
                }
                drop(link);
            });
        }
    }
}

impl Drop for MacTransport {
    fn drop(&mut self) {
        self.close();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn v2_uuid_bytes_are_big_endian() {
        let b = uuid_bytes(V2_SERVICE_UUID);
        assert_eq!(b[..4], [0x95, 0x6C, 0x7B, 0x26]);
        assert_eq!(b[12..], [0x39, 0x3C, 0xB6, 0xE2]);
    }
}
