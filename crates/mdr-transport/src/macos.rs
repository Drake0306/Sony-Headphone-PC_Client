//! macOS RFCOMM transport over IOBluetooth.
//!
//! Every IOBluetooth call runs on the main queue, and the channel delegate's
//! callbacks arrive on the main run loop, so the application's main run loop
//! must be running. Use the transport from a worker thread: blocking the main
//! thread in `recv_timeout` would starve the delegate.
//!
//! The channel is opened asynchronously: a synchronous open made inside a
//! main-queue block fails on real hardware and leaves the DLCI half-open on
//! the device (see `docs/protocol-notes.md`).

use std::ffi::c_void;
use std::io;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};

use dispatch2::DispatchQueue;
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObject, NSObjectProtocol};
use objc2::{define_class, msg_send, AllocAnyThread, DefinedClass, MainThreadMarker};
use objc2_io_bluetooth::{
    BluetoothRFCOMMChannelID, IOBluetoothDevice, IOBluetoothRFCOMMChannel,
    IOBluetoothRFCOMMChannelDelegate, IOBluetoothSDPServiceRecord, IOBluetoothSDPUUID,
};

use crate::{is_ult_wear_name, normalise_address, Candidate, Transport, V2_SERVICE_UUID};

/// Largest single `writeSync` call; larger writes are split.
const WRITE_CHUNK: usize = 512;
/// Interval between checks for the v2 record after an SDP query.
const SDP_POLL: Duration = Duration::from_millis(200);
/// How long an SDP query may take to produce the v2 record.
const SDP_WAIT: Duration = Duration::from_secs(6);
/// How long the asynchronous RFCOMM open may take to complete.
const OPEN_TIMEOUT: Duration = Duration::from_secs(15);
/// How long to wait after `closeConnection` before reopening the baseband link.
const RECONNECT_DELAY: Duration = Duration::from_millis(1500);
/// Open status reported when the channel closes before the open completes.
/// IOBluetooth never reports this value, so it cannot be mistaken for a real
/// IOReturn.
const CLOSED_BEFORE_OPEN: i32 = i32::MIN;

/// Where the delegate forwards its callbacks.
struct DelegateIvars {
    /// Received bytes; an empty `Vec` signals closure.
    data: Sender<Vec<u8>>,
    /// Status of the asynchronous open; only the first value is read.
    open: Sender<i32>,
}

define_class!(
    /// Forwards the open status and received RFCOMM data into channels; an
    /// empty `Vec` signals closure.
    #[unsafe(super(NSObject))]
    #[name = "MdrTransportRFCOMMDelegate"]
    #[ivars = DelegateIvars]
    struct Delegate;

    unsafe impl NSObjectProtocol for Delegate {}

    unsafe impl IOBluetoothRFCOMMChannelDelegate for Delegate {
        #[unsafe(method(rfcommChannelOpenComplete:status:))]
        fn open_complete(&self, _channel: Option<&IOBluetoothRFCOMMChannel>, status: i32) {
            let _ = self.ivars().open.send(status);
        }

        #[unsafe(method(rfcommChannelData:data:length:))]
        fn data(
            &self,
            _channel: Option<&IOBluetoothRFCOMMChannel>,
            data: *mut c_void,
            length: usize,
        ) {
            if data.is_null() || length == 0 {
                return;
            }
            // SAFETY: IOBluetooth passes a buffer of `length` bytes valid for this call.
            let bytes = unsafe { std::slice::from_raw_parts(data.cast::<u8>(), length) };
            let _ = self.ivars().data.send(bytes.to_vec());
        }

        #[unsafe(method(rfcommChannelClosed:))]
        fn closed(&self, _channel: Option<&IOBluetoothRFCOMMChannel>) {
            // A close before the open completed is an open failure; afterwards
            // nobody reads the open status and this send is ignored.
            let _ = self.ivars().open.send(CLOSED_BEFORE_OPEN);
            let _ = self.ivars().data.send(Vec::new());
        }
    }
);

impl Delegate {
    fn new(ivars: DelegateIvars) -> Retained<Self> {
        let this = Self::alloc().set_ivars(ivars);
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

impl<T> MainOnly<T> {
    /// Only call inside an `on_main` closure.
    fn get(&self) -> &T {
        &self.0
    }
}

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
    array
        .to_vec()
        .into_iter()
        .filter_map(|obj| obj.downcast::<IOBluetoothDevice>().ok())
        .collect()
}

fn string(s: Option<Retained<objc2_foundation::NSString>>) -> String {
    s.map(|s| s.to_string()).unwrap_or_default()
}

type Device = MainOnly<Retained<IOBluetoothDevice>>;
type Record = MainOnly<Retained<IOBluetoothSDPServiceRecord>>;

// Each helper below makes one IOBluetooth call in its own main-queue hop, so
// the main queue is never held across several calls.

fn paired() -> Vec<Device> {
    on_main(|| paired_devices().into_iter().map(MainOnly).collect())
}

fn address_of(device: &Device) -> String {
    // SAFETY: property read on the main queue.
    on_main(|| string(unsafe { device.get().addressString() }))
}

fn name_of(device: &Device) -> String {
    // SAFETY: property read on the main queue.
    on_main(|| string(unsafe { device.get().nameOrAddress() }))
}

fn is_connected(device: &Device) -> bool {
    // SAFETY: state query on the main queue.
    on_main(|| unsafe { device.get().isConnected() })
}

/// The cached v2 service record, if the device has one.
fn v2_record(device: &Device) -> io::Result<Option<Record>> {
    on_main(|| {
        let uuid = v2_uuid()?;
        // SAFETY: cached-record lookup on the main queue with a valid UUID.
        let record = unsafe { device.get().getServiceRecordForUUID(Some(&uuid)) };
        Ok(record.map(MainOnly))
    })
}

pub fn discover() -> io::Result<Vec<Candidate>> {
    let mut candidates = Vec::new();
    for device in paired() {
        let connected = is_connected(&device);
        let name = name_of(&device);
        if (connected && is_ult_wear_name(&name)) || v2_record(&device)?.is_some() {
            candidates.push(Candidate {
                name,
                address: address_of(&device),
                connected_audio: connected,
            });
        }
    }
    Ok(candidates)
}

/// Channel and delegate of an open link.
type Link = MainOnly<(Retained<IOBluetoothRFCOMMChannel>, Retained<Delegate>)>;

pub fn connect(address: &str) -> io::Result<Box<dyn Transport>> {
    // The SDP query and the RFCOMM open complete on the main run loop, which
    // cannot advance while the main thread waits for them.
    if MainThreadMarker::new().is_some() {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "connect must be called from a worker thread",
        ));
    }
    let wanted = normalise_address(address);
    let device = paired()
        .into_iter()
        .find(|d| normalise_address(&address_of(d)) == wanted)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                format!("no paired device {address}"),
            )
        })?;
    if !is_connected(&device) {
        return Err(io::Error::new(
            io::ErrorKind::NotConnected,
            "headphones not connected to this Mac",
        ));
    }
    let record = match v2_record(&device)? {
        Some(record) => record,
        None => query_v2_record(&device)?,
    };
    open_channel(&device, &record)
}

/// Asks the device for its SDP records and waits for the v2 one to appear.
///
/// The query completes on the main run loop, so the wait sleeps on this
/// (worker) thread between short main-queue polls. `connect` guarantees it
/// is not called on the main thread.
fn query_v2_record(device: &Device) -> io::Result<Record> {
    // SAFETY: starts an asynchronous query on the main queue; no target object.
    let ret = on_main(|| unsafe { device.get().performSDPQuery(None) });
    if ret != 0 {
        return Err(io_err("performSDPQuery", ret));
    }
    let not_found = || {
        io::Error::new(
            io::ErrorKind::NotFound,
            "device does not expose the v2 service",
        )
    };
    let deadline = Instant::now() + SDP_WAIT;
    while Instant::now() < deadline {
        std::thread::sleep(SDP_POLL);
        if let Some(record) = v2_record(device)? {
            return Ok(record);
        }
    }
    Err(not_found())
}

/// Opens the v2 channel. `connect` guarantees this runs off the main thread,
/// so the main run loop can deliver the open callback while we wait.
fn open_channel(device: &Device, record: &Record) -> io::Result<Box<dyn Transport>> {
    let mut channel_id: BluetoothRFCOMMChannelID = 0;
    // SAFETY: record query on the main queue; `channel_id` outlives the call.
    let ret = on_main(|| unsafe { record.get().getRFCOMMChannelID(&mut channel_id) });
    if ret != 0 {
        return Err(io_err("getRFCOMMChannelID", ret));
    }
    let (tx, rx) = mpsc::channel();
    let (open_tx, open_rx) = mpsc::channel();
    // Start the open in one short hop; never wait for the callback inside it.
    let link: Link = on_main(move || {
        let delegate = Delegate::new(DelegateIvars {
            data: tx,
            open: open_tx,
        });
        let delegate_obj: &AnyObject = &delegate;
        let mut channel = None;
        // SAFETY: IOBluetooth call on the main queue with valid arguments.
        let ret = unsafe {
            device.get().openRFCOMMChannelAsync_withChannelID_delegate(
                Some(&mut channel),
                channel_id,
                Some(delegate_obj),
            )
        };
        if ret != 0 {
            // The open never started, so the baseband link is left alone.
            if let Some(channel) = channel {
                // SAFETY: the channel holds the delegate unretained; detach it before
                // `delegate` is released at the end of this closure.
                unsafe {
                    channel.setDelegate(None);
                    channel.closeChannel();
                }
            }
            return Err(io_err("openRFCOMMChannelAsync", ret));
        }
        let channel = channel
            .ok_or_else(|| io::Error::other("openRFCOMMChannelAsync returned no channel"))?;
        Ok(MainOnly((channel, delegate)))
    })?;
    let failure = match open_rx.recv_timeout(OPEN_TIMEOUT) {
        Ok(0) => None,
        Ok(CLOSED_BEFORE_OPEN) => Some(io::Error::other(
            "RFCOMM open failed: channel closed before the open completed",
        )),
        Ok(status) => Some(io::Error::other(format!(
            "RFCOMM open failed: IOReturn {status:#010x}"
        ))),
        Err(RecvTimeoutError::Timeout) => Some(io::Error::new(
            io::ErrorKind::TimedOut,
            format!("RFCOMM open timed out after {}s", OPEN_TIMEOUT.as_secs()),
        )),
        Err(RecvTimeoutError::Disconnected) => {
            Some(io::Error::other("RFCOMM open failed: delegate went away"))
        }
    };
    if let Some(err) = failure {
        let timed_out = err.kind() == io::ErrorKind::TimedOut;
        return Err(match abandon_open(device, link, timed_out) {
            Some(note) => io::Error::new(err.kind(), format!("{err}; {note}")),
            None => err,
        });
    }
    Ok(Box::new(MacTransport {
        link: Some(link),
        rx,
        closed: false,
        channel_id,
    }))
}

/// Tears down a channel whose open failed or timed out.
///
/// A timeout (no UA, no DM) means the device may have left the DLCI
/// half-open, so the baseband connection is closed to clear it. After
/// `closeConnection` macOS does not re-establish the link on its own, so it is
/// reopened with `openConnection` (about 1-2 s). Any other failure means the
/// device answered and the DLCI is not stuck, so the link is left alone.
///
/// Returns a note on the link reset for the caller's error message, or `None`
/// when no reset was made.
fn abandon_open(device: &Device, link: Link, timed_out: bool) -> Option<String> {
    on_main(move || {
        let (channel, _delegate) = &link.0;
        // SAFETY: on the main queue. Detach the unretained delegate before the
        // link (and the delegate) is released here.
        unsafe {
            channel.setDelegate(None);
            channel.closeChannel();
            if timed_out {
                device.get().closeConnection();
            }
        }
        drop(link);
    });
    if !timed_out {
        return None;
    }
    // Wait on this worker thread so the main queue stays free.
    std::thread::sleep(RECONNECT_DELAY);
    // SAFETY: synchronous baseband open on the main queue.
    let ret = on_main(|| unsafe { device.get().openConnection() });
    Some(if ret == 0 {
        "Bluetooth link reset and reconnected".to_owned()
    } else {
        format!("Bluetooth link reset but openConnection failed: IOReturn {ret:#010x}")
    })
}

struct MacTransport {
    /// `None` once closed.
    link: Option<Link>,
    rx: Receiver<Vec<u8>>,
    closed: bool,
    /// RFCOMM channel id from the v2 SDP record.
    channel_id: BluetoothRFCOMMChannelID,
}

fn broken_pipe() -> io::Error {
    io::Error::new(io::ErrorKind::BrokenPipe, "RFCOMM channel closed")
}

impl Transport for MacTransport {
    fn write(&mut self, bytes: &[u8]) -> io::Result<()> {
        let link = self
            .link
            .as_ref()
            .filter(|_| !self.closed)
            .ok_or_else(broken_pipe)?;
        on_main(|| {
            let channel = &link.0 .0;
            // SAFETY: state query on the main queue.
            if !unsafe { channel.isOpen() } {
                return Err(broken_pipe());
            }
            for chunk in bytes.chunks(WRITE_CHUNK) {
                let mut buf = chunk.to_vec();
                // SAFETY: `buf` is valid for `len` bytes for the duration of the call; len <= 512.
                let ret =
                    unsafe { channel.writeSync_length(buf.as_mut_ptr().cast(), buf.len() as u16) };
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

    fn channel(&self) -> Option<u8> {
        Some(self.channel_id)
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
