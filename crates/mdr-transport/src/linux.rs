//! Linux RFCOMM transport: BlueZ over D-Bus (`bluer`) for discovery, a plain
//! `AF_BLUETOOTH` / `BTPROTO_RFCOMM` socket for the link.
//!
//! BlueZ no longer offers SDP browsing over D-Bus, so the RFCOMM channel comes
//! from `HAIRLINE_RFCOMM_CHANNEL` when set, else from the channel recorded for
//! this device after an earlier connect, else from a bounded scan of channels
//! 1..=30. Each candidate channel is sent exactly one frame, the protocol-info
//! GET `00 00` ([`scan_probe_frame`]); a channel that does not answer with a v2
//! reply within [`PROBE_WAIT`] is closed and the next is tried.
//!
//! The pure helpers (address bytes, channel plan, probe frame, reply check,
//! errno mapping, the scan itself over any [`Transport`]) compile on every OS
//! so they are unit-tested on the macOS host; the socket and D-Bus code is
//! Linux-only.

// Some helpers are used only by the Linux platform code and the tests.
#![cfg_attr(not(target_os = "linux"), allow(dead_code))]

use std::io;
use std::time::{Duration, Instant};

use mdr_frame::{encode, Decoder, Frame, FrameType};

use crate::{Candidate, Transport};

/// Linux errno values used by the pure mapping; checked against `libc` at
/// compile time on Linux.
pub(crate) const EINTR: i32 = 4;
pub(crate) const EAGAIN: i32 = 11;
/// The same value as `EAGAIN` on Linux; both mean the receive timed out.
pub(crate) const EWOULDBLOCK: i32 = EAGAIN;
pub(crate) const ECONNRESET: i32 = 104;
pub(crate) const ENOTCONN: i32 = 107;
pub(crate) const ETIMEDOUT: i32 = 110;
pub(crate) const ECONNREFUSED: i32 = 111;

/// Environment variable that names the RFCOMM channel and skips the scan.
pub(crate) const CHANNEL_ENV: &str = "HAIRLINE_RFCOMM_CHANNEL";
/// Highest RFCOMM channel number.
pub(crate) const MAX_CHANNEL: u8 = 30;
/// How long each scanned channel gets to answer the protocol-info GET.
pub(crate) const PROBE_WAIT: Duration = Duration::from_millis(1500);
/// Sequence number of the probe frame. The session's first frame uses 0, so
/// the headphones do not take it for a retransmission of the probe, and the
/// probe's ACK (sequence 0) does not match the session's frame in flight.
const PROBE_SEQ: u8 = 1;

/// `struct sockaddr_rc` from BlueZ `<bluetooth/rfcomm.h>`: family, the
/// address bytes in little-endian order (`bdaddr_t`), the channel.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SockaddrRc {
    pub rc_family: u16,
    pub rc_bdaddr: [u8; 6],
    pub rc_channel: u8,
}

/// `AF_BLUETOOTH`, checked against `libc` at compile time on Linux.
pub(crate) const AF_BLUETOOTH: u16 = 31;
/// `BTPROTO_RFCOMM` from BlueZ `<bluetooth/bluetooth.h>`.
pub(crate) const BTPROTO_RFCOMM: i32 = 3;

/// A Bluetooth address in any notation (`AA:BB:..`, `aa-bb-..`, bare hex) as
/// the six `bdaddr_t` bytes, least significant first, or `None` unless it has
/// exactly 12 hex digits and nothing else but separators.
pub(crate) fn bdaddr_bytes(address: &str) -> Option<[u8; 6]> {
    if !address
        .chars()
        .all(|c| c.is_ascii_hexdigit() || c == ':' || c == '-')
    {
        return None;
    }
    let hex: Vec<u8> = address.bytes().filter(u8::is_ascii_hexdigit).collect();
    if hex.len() != 12 {
        return None;
    }
    let mut out = [0u8; 6];
    for (i, pair) in hex.chunks(2).enumerate() {
        let s = std::str::from_utf8(pair).ok()?;
        out[5 - i] = u8::from_str_radix(s, 16).ok()?;
    }
    Some(out)
}

/// The socket address for this device and channel.
pub(crate) fn sockaddr_rc(bdaddr: [u8; 6], channel: u8) -> SockaddrRc {
    SockaddrRc {
        rc_family: AF_BLUETOOTH,
        rc_bdaddr: bdaddr,
        rc_channel: channel,
    }
}

/// A channel number from `HAIRLINE_RFCOMM_CHANNEL`: 1..=30, else `None`.
pub(crate) fn parse_channel(value: &str) -> Option<u8> {
    value
        .trim()
        .parse::<u8>()
        .ok()
        .filter(|c| (1..=MAX_CHANNEL).contains(c))
}

/// The order in which channels are tried: the environment's channel, then the
/// recorded one, then every other channel 1..=30 in order.
pub(crate) fn channel_plan(env: Option<u8>, recorded: Option<u8>) -> Vec<u8> {
    let valid = |c: &u8| (1..=MAX_CHANNEL).contains(c);
    let mut plan: Vec<u8> = Vec::with_capacity(MAX_CHANNEL as usize);
    for c in [env, recorded].into_iter().flatten().filter(valid) {
        if !plan.contains(&c) {
            plan.push(c);
        }
    }
    let first = plan.clone();
    plan.extend((1..=MAX_CHANNEL).filter(|c| !first.contains(c)));
    plan
}

/// Where the channel that answered came from, for the log line.
pub(crate) fn channel_source(channel: u8, env: Option<u8>, recorded: Option<u8>) -> &'static str {
    if env == Some(channel) {
        CHANNEL_ENV
    } else if recorded == Some(channel) {
        "recorded channel"
    } else {
        "channel scan"
    }
}

/// The one frame the channel scan sends: the T1 protocol-info GET `00 00`,
/// framed by the normal encoder.
pub(crate) fn scan_probe_frame() -> Vec<u8> {
    encode(&Frame {
        frame_type: FrameType::Command1,
        seq: PROBE_SEQ,
        payload: vec![0x00, 0x00],
    })
}

/// Whether a T1 payload is the v2 protocol-info reply: `01 00 …`, 8 bytes
/// long (`docs/protocol-notes.md`, handshake step 1).
pub(crate) fn is_v2_protocol_reply(payload: &[u8]) -> bool {
    payload.len() == 8 && payload.starts_with(&[0x01, 0x00])
}

/// An errno as an `io::Error` with the matching kind.
pub(crate) fn map_errno(errno: i32, what: &str) -> io::Error {
    let kind = match errno {
        EAGAIN | ETIMEDOUT => io::ErrorKind::TimedOut,
        ECONNRESET => io::ErrorKind::ConnectionReset,
        ENOTCONN => io::ErrorKind::NotConnected,
        ECONNREFUSED => io::ErrorKind::ConnectionRefused,
        EINTR => io::ErrorKind::Interrupted,
        _ => io::ErrorKind::Other,
    };
    io::Error::new(kind, format!("{what} failed: errno {errno}"))
}

/// Maps a `recv` return value and errno to the `Transport` contract: bytes
/// read, `Ok(None)` on timeout (or an interrupted wait), `Err` on link loss.
/// A zero return means the peer closed the link.
pub(crate) fn map_recv(result: isize, errno: i32) -> io::Result<Option<usize>> {
    match result {
        n if n > 0 => Ok(Some(n as usize)),
        0 => Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "headphones closed the RFCOMM link",
        )),
        _ => match errno {
            // EWOULDBLOCK is EAGAIN on Linux.
            EAGAIN | EINTR => Ok(None),
            ECONNRESET => Err(io::Error::new(
                io::ErrorKind::ConnectionReset,
                "RFCOMM link reset",
            )),
            ENOTCONN => Err(io::Error::new(
                io::ErrorKind::NotConnected,
                "RFCOMM link not connected",
            )),
            code => Err(map_errno(code, "recv")),
        },
    }
}

/// Whether a failed RFCOMM `connect` means "nothing listens on this channel"
/// (try the next one) rather than a fault that ends the scan.
pub(crate) fn connect_refused(errno: i32) -> bool {
    errno == ECONNREFUSED
}

/// A discovered device, its address rendered `aa:bb:cc:dd:ee:ff`.
pub(crate) fn candidate_from(name: &str, address: &str, connected: bool) -> Candidate {
    Candidate {
        name: name.to_string(),
        address: address.to_ascii_lowercase(),
        connected_audio: connected,
    }
}

/// Refuses a device that is paired but has no connection to this computer.
pub(crate) fn check_connected(candidate: &Candidate) -> io::Result<()> {
    if candidate.connected_audio {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::NotConnected,
            "headphones not connected to this PC",
        ))
    }
}

/// The channel that answered, its open link, and every byte received while
/// probing it (the protocol-info reply among them), for the session to read.
pub(crate) struct Found<L> {
    pub channel: u8,
    pub link: L,
    pub buffered: Vec<u8>,
}

/// Sends the probe on an open channel and waits up to `wait` for the v2
/// protocol-info reply. `Some(bytes received)` when it came, else `None`.
/// This is the only place the scan writes, and it writes only
/// [`scan_probe_frame`].
fn probe<L: Transport>(link: &mut L, wait: Duration) -> Option<Vec<u8>> {
    link.write(&scan_probe_frame()).ok()?;
    let deadline = Instant::now() + wait;
    let mut decoder = Decoder::new();
    let mut buffered = Vec::new();
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return None;
        }
        let bytes = match link.recv_timeout(left) {
            Ok(Some(bytes)) => bytes,
            Ok(None) => continue,
            Err(_) => return None,
        };
        buffered.extend_from_slice(&bytes);
        let v2 = decoder
            .push(&bytes)
            .into_iter()
            .flatten()
            .any(|f| f.frame_type == FrameType::Command1 && is_v2_protocol_reply(&f.payload));
        if v2 {
            return Some(buffered);
        }
    }
}

/// Tries the channels in `plan` in order. `open` connects one channel:
/// `Ok(None)` when it is refused (try the next), `Err` for a fault that ends
/// the scan. A channel that does not answer the probe is closed.
pub(crate) fn scan<L: Transport>(
    plan: &[u8],
    open: &mut dyn FnMut(u8) -> io::Result<Option<L>>,
    wait: Duration,
) -> io::Result<Found<L>> {
    for &channel in plan {
        let Some(mut link) = open(channel)? else {
            continue;
        };
        match probe(&mut link, wait) {
            Some(buffered) => {
                return Ok(Found {
                    channel,
                    link,
                    buffered,
                })
            }
            None => link.close(),
        }
    }
    Err(io::Error::new(
        io::ErrorKind::NotFound,
        "no RFCOMM channel answered the MDR v2 protocol",
    ))
}

/// Refuses an unconnected device before any socket is opened, then scans.
pub(crate) fn connect_candidate<L: Transport>(
    candidate: &Candidate,
    plan: &[u8],
    open: &mut dyn FnMut(u8) -> io::Result<Option<L>>,
    wait: Duration,
) -> io::Result<Found<L>> {
    check_connected(candidate)?;
    scan(plan, open, wait)
}

#[cfg(target_os = "linux")]
pub(crate) use platform::{connect_with_hint, discover};

#[cfg(target_os = "linux")]
mod platform {
    use std::io;
    use std::mem;
    use std::time::Duration;

    use super::{
        bdaddr_bytes, candidate_from, channel_plan, channel_source, connect_candidate,
        connect_refused, map_errno, map_recv, parse_channel, sockaddr_rc, SockaddrRc,
        BTPROTO_RFCOMM, CHANNEL_ENV, PROBE_WAIT,
    };
    use crate::{is_ult_wear_name, Candidate, Transport};

    /// Size of the buffer each `recv` call reads into.
    const RECV_BUFFER: usize = 1024;

    const _: () = {
        assert!(super::EINTR == libc::EINTR);
        assert!(super::EAGAIN == libc::EAGAIN);
        assert!(super::EWOULDBLOCK == libc::EWOULDBLOCK);
        assert!(super::ECONNRESET == libc::ECONNRESET);
        assert!(super::ENOTCONN == libc::ENOTCONN);
        assert!(super::ETIMEDOUT == libc::ETIMEDOUT);
        assert!(super::ECONNREFUSED == libc::ECONNREFUSED);
        assert!(super::AF_BLUETOOTH as i32 == libc::AF_BLUETOOTH);
        assert!(mem::size_of::<libc::sa_family_t>() == 2);
    };

    fn errno() -> i32 {
        io::Error::last_os_error().raw_os_error().unwrap_or(0)
    }

    fn bt_error(e: bluer::Error) -> io::Error {
        io::Error::other(format!("BlueZ: {e}"))
    }

    /// Every paired device BlueZ knows on the default adapter, read over D-Bus
    /// on a short-lived current-thread runtime.
    fn paired() -> io::Result<Vec<Candidate>> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        runtime.block_on(async {
            let session = bluer::Session::new().await.map_err(bt_error)?;
            let adapter = session.default_adapter().await.map_err(bt_error)?;
            let mut devices = Vec::new();
            for addr in adapter.device_addresses().await.map_err(bt_error)? {
                let device = adapter.device(addr).map_err(bt_error)?;
                if !device.is_paired().await.unwrap_or(false) {
                    continue;
                }
                let name = match device.name().await {
                    Ok(Some(name)) => name,
                    _ => device.alias().await.unwrap_or_default(),
                };
                let connected = device.is_connected().await.unwrap_or(false);
                devices.push(candidate_from(&name, &addr.to_string(), connected));
            }
            Ok(devices)
        })
    }

    pub(crate) fn discover() -> io::Result<Vec<Candidate>> {
        Ok(paired()?
            .into_iter()
            .filter(|c| is_ult_wear_name(&c.name))
            .collect())
    }

    pub(crate) fn connect_with_hint(
        address: &str,
        recorded: Option<u8>,
    ) -> io::Result<Box<dyn Transport>> {
        let bdaddr = bdaddr_bytes(address).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("not a Bluetooth address: {address}"),
            )
        })?;
        let candidate = paired()?
            .into_iter()
            .find(|c| bdaddr_bytes(&c.address) == Some(bdaddr))
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::NotFound,
                    format!("no paired device {address}"),
                )
            })?;
        let env = std::env::var(CHANNEL_ENV)
            .ok()
            .and_then(|v| parse_channel(&v));
        let plan = channel_plan(env, recorded);
        let found = connect_candidate(
            &candidate,
            &plan,
            &mut |channel| open_channel(bdaddr, channel),
            PROBE_WAIT,
        )?;
        eprintln!(
            "hairline: RFCOMM channel {} ({})",
            found.channel,
            channel_source(found.channel, env, recorded)
        );
        let mut link = found.link;
        link.pending = Some(found.buffered);
        Ok(Box::new(link))
    }

    /// Opens an RFCOMM socket to the channel: `Ok(None)` when the channel
    /// refuses the connection.
    fn open_channel(bdaddr: [u8; 6], channel: u8) -> io::Result<Option<LinuxTransport>> {
        // SAFETY: plain socket creation.
        let fd = unsafe {
            libc::socket(
                libc::AF_BLUETOOTH,
                libc::SOCK_STREAM | libc::SOCK_CLOEXEC,
                BTPROTO_RFCOMM,
            )
        };
        if fd < 0 {
            return Err(map_errno(errno(), "socket(AF_BLUETOOTH)"));
        }
        // Owns the socket from here, so every early return closes it.
        let link = LinuxTransport {
            fd,
            channel,
            pending: None,
        };
        let target: SockaddrRc = sockaddr_rc(bdaddr, channel);
        // SAFETY: `target` is a valid sockaddr_rc of the given size.
        let ret = unsafe {
            libc::connect(
                fd,
                (&target as *const SockaddrRc).cast::<libc::sockaddr>(),
                mem::size_of::<SockaddrRc>() as libc::socklen_t,
            )
        };
        if ret != 0 {
            let code = errno();
            if connect_refused(code) {
                return Ok(None);
            }
            return Err(map_errno(
                code,
                &format!("RFCOMM connect (channel {channel})"),
            ));
        }
        Ok(Some(link))
    }

    pub(super) struct LinuxTransport {
        fd: libc::c_int,
        channel: u8,
        /// Bytes received during the channel probe, returned by the first
        /// `recv_timeout` so the session sees the protocol-info reply.
        pub(super) pending: Option<Vec<u8>>,
    }

    impl Transport for LinuxTransport {
        fn write(&mut self, bytes: &[u8]) -> io::Result<()> {
            let mut rest = bytes;
            while !rest.is_empty() {
                // SAFETY: `rest` is readable for its length.
                let sent = unsafe {
                    libc::send(
                        self.fd,
                        rest.as_ptr().cast(),
                        rest.len(),
                        libc::MSG_NOSIGNAL,
                    )
                };
                if sent < 0 {
                    let code = errno();
                    if code == libc::EINTR {
                        continue;
                    }
                    return Err(map_errno(code, "send"));
                }
                rest = &rest[sent as usize..];
            }
            Ok(())
        }

        fn recv_timeout(&mut self, timeout: Duration) -> io::Result<Option<Vec<u8>>> {
            if let Some(bytes) = self.pending.take().filter(|b| !b.is_empty()) {
                return Ok(Some(bytes));
            }
            // A zero timeval means "wait forever", so wait at least 1 ms.
            let timeout = timeout.max(Duration::from_millis(1));
            let tv = libc::timeval {
                tv_sec: timeout.as_secs().min(i32::MAX as u64) as libc::time_t,
                tv_usec: timeout.subsec_micros() as libc::suseconds_t,
            };
            // SAFETY: `tv` is a readable timeval, as SO_RCVTIMEO expects.
            let ret = unsafe {
                libc::setsockopt(
                    self.fd,
                    libc::SOL_SOCKET,
                    libc::SO_RCVTIMEO,
                    (&tv as *const libc::timeval).cast(),
                    mem::size_of::<libc::timeval>() as libc::socklen_t,
                )
            };
            if ret != 0 {
                return Err(map_errno(errno(), "setsockopt(SO_RCVTIMEO)"));
            }
            let mut buf = [0u8; RECV_BUFFER];
            // SAFETY: `buf` is writable for its length.
            let n = unsafe { libc::recv(self.fd, buf.as_mut_ptr().cast(), buf.len(), 0) };
            let code = if n < 0 { errno() } else { 0 };
            Ok(map_recv(n, code)?.map(|len| buf[..len].to_vec()))
        }

        fn close(&mut self) {
            if self.fd < 0 {
                return;
            }
            // SAFETY: the socket is open and closed exactly once.
            unsafe {
                libc::shutdown(self.fd, libc::SHUT_RDWR);
                libc::close(self.fd);
            }
            self.fd = -1;
        }

        fn channel(&self) -> Option<u8> {
            Some(self.channel)
        }
    }

    impl Drop for LinuxTransport {
        fn drop(&mut self) {
            self.close();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::normalise_address;
    use std::collections::VecDeque;
    use std::sync::{Arc, Mutex};

    type Log<T> = Arc<Mutex<Vec<T>>>;

    fn log<T>() -> Log<T> {
        Arc::new(Mutex::new(Vec::new()))
    }

    /// A channel in a scripted scan: every write is logged; `recv_timeout`
    /// returns the scripted chunks, then times out.
    struct ScriptedLink {
        writes: Log<Vec<u8>>,
        replies: VecDeque<Vec<u8>>,
        closed: Log<u8>,
        channel: u8,
    }

    impl Transport for ScriptedLink {
        fn write(&mut self, bytes: &[u8]) -> io::Result<()> {
            self.writes.lock().unwrap().push(bytes.to_vec());
            Ok(())
        }
        fn recv_timeout(&mut self, _timeout: Duration) -> io::Result<Option<Vec<u8>>> {
            Ok(self.replies.pop_front())
        }
        fn close(&mut self) {
            self.closed.lock().unwrap().push(self.channel);
        }
    }

    fn v2_reply_bytes() -> Vec<u8> {
        encode(&Frame {
            frame_type: FrameType::Command1,
            seq: 0,
            payload: vec![0x01, 0x00, 0x03, 0x00, 0x20, 0x02, 0x00, 0x00],
        })
    }

    #[test]
    fn bdaddr_is_little_endian() {
        let bytes = [0x54, 0xD6, 0xFE, 0xE8, 0xE6, 0xA8];
        let expected = Some(bytes);
        assert_eq!(bdaddr_bytes("A8:E6:E8:FE:D6:54"), expected);
        assert_eq!(bdaddr_bytes("a8-e6-e8-fe-d6-54"), expected);
        assert_eq!(bdaddr_bytes("A8E6E8FED654"), expected);
        assert_eq!(bdaddr_bytes("A8:E6:E8:FE:D6"), None);
        assert_eq!(bdaddr_bytes("A8:E6:E8:FE:D6:54:00"), None);
        assert_eq!(bdaddr_bytes("ZZ:E6:E8:FE:D6:54"), None);
        assert_eq!(bdaddr_bytes(""), None);

        // sockaddr_rc: family u16 at 0, bdaddr 6 bytes at 2, channel u8 at 8.
        assert_eq!(std::mem::offset_of!(SockaddrRc, rc_family), 0);
        assert_eq!(std::mem::offset_of!(SockaddrRc, rc_bdaddr), 2);
        assert_eq!(std::mem::offset_of!(SockaddrRc, rc_channel), 8);
        assert_eq!(std::mem::size_of::<SockaddrRc>(), 10);
        let sa = sockaddr_rc(bytes, 18);
        assert_eq!(sa.rc_family, 31);
        assert_eq!(sa.rc_bdaddr, [0x54, 0xD6, 0xFE, 0xE8, 0xE6, 0xA8]);
        assert_eq!(sa.rc_channel, 18);
    }

    #[test]
    fn channel_plan_order() {
        let all: Vec<u8> = (1..=30).collect();
        assert_eq!(channel_plan(None, None), all);

        let plan = channel_plan(Some(5), Some(18));
        assert_eq!(&plan[..2], &[5, 18]);
        assert_eq!(plan.len(), 30);
        let rest: Vec<u8> = (1..=30).filter(|c| *c != 5 && *c != 18).collect();
        assert_eq!(&plan[2..], &rest[..]);

        // Same channel twice is tried once; out-of-range hints are ignored.
        let plan = channel_plan(Some(18), Some(18));
        assert_eq!(plan[0], 18);
        assert_eq!(plan.len(), 30);
        assert_eq!(channel_plan(Some(0), Some(31)), all);

        assert_eq!(parse_channel("18"), Some(18));
        assert_eq!(parse_channel(" 7 "), Some(7));
        assert_eq!(parse_channel("0"), None);
        assert_eq!(parse_channel("31"), None);
        assert_eq!(parse_channel("x"), None);

        assert_eq!(channel_source(5, Some(5), Some(18)), CHANNEL_ENV);
        assert_eq!(channel_source(18, Some(5), Some(18)), "recorded channel");
        assert_eq!(channel_source(3, Some(5), Some(18)), "channel scan");
    }

    #[test]
    fn recorded_channel_skips_the_scan() {
        let plan = channel_plan(None, Some(18));
        assert_eq!(plan[0], 18);

        // The recorded channel answers: no other channel is opened.
        let writes = log();
        let closed = log();
        let mut opened = Vec::new();
        let found = scan(
            &plan,
            &mut |ch| {
                opened.push(ch);
                Ok(Some(ScriptedLink {
                    writes: writes.clone(),
                    replies: VecDeque::from([v2_reply_bytes()]),
                    closed: closed.clone(),
                    channel: ch,
                }))
            },
            Duration::from_millis(50),
        )
        .unwrap();
        assert_eq!(found.channel, 18);
        assert_eq!(opened, vec![18]);
        assert_eq!(found.buffered, v2_reply_bytes());
        assert!(closed.lock().unwrap().is_empty());
    }

    #[test]
    fn scan_sends_only_protocol_info_get() {
        // The probe frame is the T1 GET `00 00` and nothing else.
        let frames = Decoder::new().push(&scan_probe_frame());
        assert_eq!(frames.len(), 1);
        let frame = frames[0].as_ref().unwrap();
        assert_eq!(frame.frame_type, FrameType::Command1);
        assert_eq!(frame.payload, vec![0x00, 0x00]);

        // A full scan where channels refuse, stay silent, answer junk, answer
        // a non-v2 protocol reply, and finally channel 9 answers v2: every
        // byte written anywhere is exactly one probe frame per opened channel.
        let writes = log();
        let closed = log();
        let junk = vec![0xFF, 0x00, 0x3E];
        let v1 = encode(&Frame {
            frame_type: FrameType::Command1,
            seq: 0,
            payload: vec![0x01, 0x00, 0x02],
        });
        let mut opened = Vec::new();
        let found = scan(
            &channel_plan(None, None),
            &mut |ch| {
                let replies = match ch {
                    1 | 2 => return Ok(None),
                    3 => VecDeque::new(),
                    4 => VecDeque::from([junk.clone()]),
                    5 => VecDeque::from([v1.clone()]),
                    _ => VecDeque::from([v2_reply_bytes()]),
                };
                if ch > 9 {
                    panic!("scan went past the channel that answered");
                }
                opened.push(ch);
                Ok(Some(ScriptedLink {
                    writes: writes.clone(),
                    replies,
                    closed: closed.clone(),
                    channel: ch,
                }))
            },
            Duration::from_millis(30),
        )
        .unwrap();
        assert_eq!(found.channel, 6);
        assert_eq!(opened, vec![3, 4, 5, 6]);
        assert_eq!(*closed.lock().unwrap(), vec![3, 4, 5]);
        let writes = writes.lock().unwrap();
        assert_eq!(writes.len(), opened.len());
        assert!(writes.iter().all(|w| *w == scan_probe_frame()));

        // Nothing answers: an error, and still only probes were written.
        let writes = log();
        let err = scan(
            &[1, 2, 3],
            &mut |ch| {
                Ok(Some(ScriptedLink {
                    writes: writes.clone(),
                    replies: VecDeque::new(),
                    closed: closed.clone(),
                    channel: ch,
                }))
            },
            Duration::from_millis(10),
        )
        .err()
        .unwrap();
        assert_eq!(err.kind(), io::ErrorKind::NotFound);
        assert_eq!(
            err.to_string(),
            "no RFCOMM channel answered the MDR v2 protocol"
        );
        assert_eq!(writes.lock().unwrap().len(), 3);
        assert!(writes
            .lock()
            .unwrap()
            .iter()
            .all(|w| *w == scan_probe_frame()));
    }

    #[test]
    fn v2_reply_check() {
        assert!(is_v2_protocol_reply(&[
            0x01, 0x00, 0x03, 0x00, 0x20, 0x02, 0x00, 0x00
        ]));
        assert!(!is_v2_protocol_reply(&[0x01, 0x00, 0x02]));
        assert!(!is_v2_protocol_reply(&[
            0x07, 0x00, 0x03, 0x00, 0x20, 0x02, 0x00, 0x00
        ]));
        assert!(!is_v2_protocol_reply(&[]));
    }

    #[test]
    fn connect_refuses_unconnected_candidate() {
        let paired = candidate_from("ULT WEAR", "A8:E6:E8:FE:D6:54", false);
        assert!(!paired.connected_audio);
        let mut opened = 0;
        let err = connect_candidate::<ScriptedLink>(
            &paired,
            &channel_plan(None, Some(18)),
            &mut |_| {
                opened += 1;
                Ok(None)
            },
            Duration::from_millis(10),
        )
        .err()
        .unwrap();
        assert_eq!(err.kind(), io::ErrorKind::NotConnected);
        assert_eq!(
            opened, 0,
            "no socket may be opened for an unconnected device"
        );

        let connected = candidate_from("ULT WEAR", "A8:E6:E8:FE:D6:54", true);
        assert!(check_connected(&connected).is_ok());
    }

    #[test]
    fn linux_error_mapping() {
        assert_eq!(map_recv(5, 0).unwrap(), Some(5));
        assert_eq!(map_recv(-1, EAGAIN).unwrap(), None);
        assert_eq!(map_recv(-1, EWOULDBLOCK).unwrap(), None);
        assert_eq!(map_recv(-1, EINTR).unwrap(), None);
        let reset = map_recv(-1, ECONNRESET).unwrap_err();
        assert_eq!(reset.kind(), io::ErrorKind::ConnectionReset);
        let not_conn = map_recv(-1, ENOTCONN).unwrap_err();
        assert_eq!(not_conn.kind(), io::ErrorKind::NotConnected);
        let closed = map_recv(0, 0).unwrap_err();
        assert_eq!(closed.kind(), io::ErrorKind::UnexpectedEof);
        assert!(map_recv(-1, 113).is_err());

        assert!(connect_refused(ECONNREFUSED));
        assert!(!connect_refused(ETIMEDOUT));
        assert_eq!(
            map_errno(ECONNREFUSED, "connect").kind(),
            io::ErrorKind::ConnectionRefused
        );
        assert_eq!(
            map_errno(ETIMEDOUT, "connect").kind(),
            io::ErrorKind::TimedOut
        );
        assert_eq!(
            map_errno(113, "connect").to_string(),
            "connect failed: errno 113"
        );
    }

    #[test]
    fn candidate_from_renders_colon_lowercase() {
        let c = candidate_from("ULT WEAR", "A8:E6:E8:FE:D6:54", true);
        assert_eq!(c.address, "a8:e6:e8:fe:d6:54");
        assert_eq!(
            normalise_address(&c.address),
            normalise_address("A8-E6-E8-FE-D6-54")
        );
    }
}
