//! Windows RFCOMM transport over Winsock (`AF_BTH` / `BTHPROTO_RFCOMM`).
//!
//! `discover` walks the paired devices with `BluetoothFindFirstDevice`;
//! `connect` opens a stream socket to `SOCKADDR_BTH` with the v2 service UUID
//! and port 0, so the Bluetooth stack resolves the RFCOMM channel from SDP.
//!
//! The pure helpers (address parsing, GUID layout, error mapping) compile on
//! every OS so they are unit-tested on the macOS host; the socket code is
//! Windows-only.

use std::io;

use crate::{Candidate, V2_SERVICE_UUID};

/// Winsock values used by the pure mapping; checked against `windows-sys` at
/// compile time on Windows.
pub(crate) const SOCKET_ERROR: i32 = -1;
pub(crate) const WSAETIMEDOUT: i32 = 10060;
pub(crate) const WSAECONNRESET: i32 = 10054;
pub(crate) const WSAENOTCONN: i32 = 10057;

/// A Windows `GUID` in its native field layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Guid {
    pub data1: u32,
    pub data2: u16,
    pub data3: u16,
    pub data4: [u8; 8],
}

/// A Bluetooth address in any notation (`AA:BB:..`, `aa-bb-..`, bare hex) as
/// the `BTH_ADDR` integer, or `None` unless it has exactly 12 hex digits and
/// nothing else but separators.
pub(crate) fn bth_addr(address: &str) -> Option<u64> {
    if !address
        .chars()
        .all(|c| c.is_ascii_hexdigit() || c == ':' || c == '-')
    {
        return None;
    }
    let hex: String = address.chars().filter(char::is_ascii_hexdigit).collect();
    if hex.len() != 12 {
        return None;
    }
    u64::from_str_radix(&hex, 16).ok()
}

/// The v2 service UUID laid out as a Windows `GUID`.
pub(crate) fn v2_guid() -> Guid {
    let hex: Vec<u8> = V2_SERVICE_UUID
        .bytes()
        .filter(u8::is_ascii_hexdigit)
        .collect();
    assert_eq!(hex.len(), 32, "UUID must have 32 hex digits");
    let mut bytes = [0u8; 16];
    for (i, pair) in hex.chunks(2).enumerate() {
        let s = std::str::from_utf8(pair).expect("ASCII hex");
        bytes[i] = u8::from_str_radix(s, 16).expect("hex digit");
    }
    let mut data4 = [0u8; 8];
    data4.copy_from_slice(&bytes[8..]);
    Guid {
        data1: u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]),
        data2: u16::from_be_bytes([bytes[4], bytes[5]]),
        data3: u16::from_be_bytes([bytes[6], bytes[7]]),
        data4,
    }
}

/// Maps a `recv` return value and `WSAGetLastError` to the `Transport`
/// contract: bytes read, `Ok(None)` on timeout, `Err` on link loss. A zero
/// return means the peer closed the link.
pub(crate) fn map_recv(result: i32, err: i32) -> io::Result<Option<usize>> {
    match result {
        n if n > 0 => Ok(Some(n as usize)),
        0 => Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "headphones closed the RFCOMM link",
        )),
        _ => match err {
            WSAETIMEDOUT => Ok(None),
            WSAECONNRESET => Err(io::Error::new(
                io::ErrorKind::ConnectionReset,
                "RFCOMM link reset",
            )),
            WSAENOTCONN => Err(io::Error::new(
                io::ErrorKind::NotConnected,
                "RFCOMM link not connected",
            )),
            code => Err(io::Error::other(format!(
                "recv failed: Winsock error {code}"
            ))),
        },
    }
}

/// A discovered device, its address rendered `aa:bb:cc:dd:ee:ff`.
pub(crate) fn candidate_from(name: &str, address: u64, connected: bool) -> Candidate {
    let b = address.to_be_bytes();
    Candidate {
        name: name.to_string(),
        address: format!(
            "{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
            b[2], b[3], b[4], b[5], b[6], b[7]
        ),
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

#[cfg(windows)]
pub(crate) use platform::{connect, discover};

#[cfg(windows)]
mod platform {
    use std::io;
    use std::mem;
    use std::sync::OnceLock;
    use std::time::Duration;

    use windows_sys::core::GUID;
    use windows_sys::Win32::Devices::Bluetooth::{
        BluetoothFindDeviceClose, BluetoothFindFirstDevice, BluetoothFindNextDevice, AF_BTH,
        BLUETOOTH_DEVICE_INFO, BLUETOOTH_DEVICE_SEARCH_PARAMS, BTHPROTO_RFCOMM, SOCKADDR_BTH,
    };
    use windows_sys::Win32::Networking::WinSock::{
        closesocket, connect as ws_connect, getpeername, recv, send, setsockopt, shutdown, socket,
        WSAGetLastError, WSAStartup, INVALID_SOCKET, SD_BOTH, SOCKADDR, SOCKET, SOCK_STREAM,
        SOL_SOCKET, SO_RCVTIMEO, WSADATA,
    };

    use super::{bth_addr, candidate_from, check_connected, map_recv, v2_guid};
    use crate::{is_ult_wear_name, Candidate, Transport};

    /// Size of the buffer each `recv` call reads into.
    const RECV_BUFFER: usize = 1024;

    const _: () = {
        use windows_sys::Win32::Networking::WinSock as ws;
        assert!(super::SOCKET_ERROR == ws::SOCKET_ERROR);
        assert!(super::WSAETIMEDOUT == ws::WSAETIMEDOUT);
        assert!(super::WSAECONNRESET == ws::WSAECONNRESET);
        assert!(super::WSAENOTCONN == ws::WSAENOTCONN);
    };

    fn last_error(what: &str) -> io::Error {
        // SAFETY: no preconditions.
        let code = unsafe { WSAGetLastError() };
        io::Error::other(format!("{what} failed: Winsock error {code}"))
    }

    /// Result of the one `WSAStartup` call this process makes. Winsock is never
    /// cleaned up, so a live transport can never lose it.
    static WSA: OnceLock<Result<(), i32>> = OnceLock::new();

    pub(super) fn wsa_init() -> io::Result<()> {
        let result = WSA.get_or_init(|| {
            // SAFETY: WSADATA is plain data that WSAStartup fills in.
            let mut data: WSADATA = unsafe { mem::zeroed() };
            // SAFETY: valid out-pointer; requests Winsock 2.2.
            match unsafe { WSAStartup(0x0202, &mut data) } {
                0 => Ok(()),
                code => Err(code),
            }
        });
        result.map_err(|code| io::Error::other(format!("WSAStartup failed: error {code}")))
    }

    /// Whether `WSAStartup` has run (successfully or not) in this process.
    #[cfg(test)]
    pub(super) fn wsa_initialised() -> bool {
        WSA.get().is_some()
    }

    fn name_of(info: &BLUETOOTH_DEVICE_INFO) -> String {
        let len = info
            .szName
            .iter()
            .position(|&c| c == 0)
            .unwrap_or(info.szName.len());
        String::from_utf16_lossy(&info.szName[..len])
    }

    fn new_info() -> BLUETOOTH_DEVICE_INFO {
        // SAFETY: BLUETOOTH_DEVICE_INFO is plain data; zero is a valid value.
        let mut info: BLUETOOTH_DEVICE_INFO = unsafe { mem::zeroed() };
        info.dwSize = mem::size_of::<BLUETOOTH_DEVICE_INFO>() as u32;
        info
    }

    /// Every paired (authenticated, remembered or connected) device, without
    /// issuing an inquiry.
    fn paired() -> Vec<Candidate> {
        // SAFETY: plain data; zero means "all radios" for `hRadio`.
        let mut params: BLUETOOTH_DEVICE_SEARCH_PARAMS = unsafe { mem::zeroed() };
        params.dwSize = mem::size_of::<BLUETOOTH_DEVICE_SEARCH_PARAMS>() as u32;
        params.fReturnAuthenticated = 1;
        params.fReturnConnected = 1;
        params.fReturnRemembered = 1;
        params.fReturnUnknown = 0;
        params.fIssueInquiry = 0;

        let mut devices = Vec::new();
        let mut info = new_info();
        // SAFETY: both pointers are valid and dwSize is set.
        let find = unsafe { BluetoothFindFirstDevice(&params, &mut info) };
        if find.is_null() {
            return devices;
        }
        loop {
            // SAFETY: `Address` is a union over the same 8 bytes; `ullLong` is
            // always initialised by the API.
            let address = unsafe { info.Address.Anonymous.ullLong };
            devices.push(candidate_from(
                &name_of(&info),
                address,
                info.fConnected != 0,
            ));
            info = new_info();
            // SAFETY: `find` is a live handle and `info` has dwSize set.
            if unsafe { BluetoothFindNextDevice(find, &mut info) } == 0 {
                break;
            }
        }
        // SAFETY: `find` is a live handle, closed once.
        unsafe { BluetoothFindDeviceClose(find) };
        devices
    }

    pub(crate) fn discover() -> io::Result<Vec<Candidate>> {
        Ok(paired()
            .into_iter()
            .filter(|c| is_ult_wear_name(&c.name))
            .collect())
    }

    pub(crate) fn connect(address: &str) -> io::Result<Box<dyn Transport>> {
        let bt_addr = bth_addr(address).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("not a Bluetooth address: {address}"),
            )
        })?;
        let candidate = paired()
            .into_iter()
            .find(|c| bth_addr(&c.address) == Some(bt_addr))
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::NotFound,
                    format!("no paired device {address}"),
                )
            })?;
        check_connected(&candidate)?;
        wsa_init()?;

        // SAFETY: plain socket creation.
        let sock = unsafe { socket(AF_BTH as i32, SOCK_STREAM, BTHPROTO_RFCOMM as i32) };
        if sock == INVALID_SOCKET {
            return Err(last_error("socket"));
        }
        // Owns the socket from here, so every early return closes it.
        let mut transport = WinTransport {
            socket: sock,
            channel: None,
        };

        let guid = v2_guid();
        let target = SOCKADDR_BTH {
            addressFamily: AF_BTH,
            btAddr: bt_addr,
            serviceClassId: GUID {
                data1: guid.data1,
                data2: guid.data2,
                data3: guid.data3,
                data4: guid.data4,
            },
            port: 0,
        };
        // SAFETY: `target` is a valid SOCKADDR_BTH of the given size.
        let ret = unsafe {
            ws_connect(
                sock,
                (&target as *const SOCKADDR_BTH).cast::<SOCKADDR>(),
                mem::size_of::<SOCKADDR_BTH>() as i32,
            )
        };
        if ret != 0 {
            return Err(last_error("RFCOMM connect"));
        }
        transport.channel = peer_port(sock);
        Ok(Box::new(transport))
    }

    /// The RFCOMM channel the stack resolved, if it reports one.
    fn peer_port(sock: SOCKET) -> Option<u8> {
        // SAFETY: plain data.
        let mut peer: SOCKADDR_BTH = unsafe { mem::zeroed() };
        let mut len = mem::size_of::<SOCKADDR_BTH>() as i32;
        // SAFETY: `peer` is writable for `len` bytes.
        let ret = unsafe {
            getpeername(
                sock,
                (&mut peer as *mut SOCKADDR_BTH).cast::<SOCKADDR>(),
                &mut len,
            )
        };
        if ret != 0 || peer.port == 0 {
            return None;
        }
        u8::try_from(peer.port).ok()
    }

    struct WinTransport {
        socket: SOCKET,
        channel: Option<u8>,
    }

    impl Transport for WinTransport {
        fn write(&mut self, bytes: &[u8]) -> io::Result<()> {
            let mut rest = bytes;
            while !rest.is_empty() {
                let len = rest.len().min(i32::MAX as usize) as i32;
                // SAFETY: `rest` is readable for `len` bytes.
                let sent = unsafe { send(self.socket, rest.as_ptr(), len, 0) };
                if sent <= 0 {
                    return Err(last_error("send"));
                }
                rest = &rest[sent as usize..];
            }
            Ok(())
        }

        fn recv_timeout(&mut self, timeout: Duration) -> io::Result<Option<Vec<u8>>> {
            // Zero means "wait forever" to Winsock, so wait at least 1 ms.
            let ms = u32::try_from(timeout.as_millis())
                .unwrap_or(u32::MAX)
                .max(1);
            // SAFETY: `ms` is a readable DWORD, as SO_RCVTIMEO expects.
            let ret = unsafe {
                setsockopt(
                    self.socket,
                    SOL_SOCKET,
                    SO_RCVTIMEO,
                    (&ms as *const u32).cast::<u8>(),
                    mem::size_of::<u32>() as i32,
                )
            };
            if ret != 0 {
                return Err(last_error("setsockopt(SO_RCVTIMEO)"));
            }
            let mut buf = [0u8; RECV_BUFFER];
            // SAFETY: `buf` is writable for its length.
            let n = unsafe { recv(self.socket, buf.as_mut_ptr(), buf.len() as i32, 0) };
            // SAFETY: no preconditions; only read when recv failed.
            let err = if n < 0 {
                unsafe { WSAGetLastError() }
            } else {
                0
            };
            Ok(map_recv(n, err)?.map(|len| buf[..len].to_vec()))
        }

        fn close(&mut self) {
            if self.socket == INVALID_SOCKET {
                return;
            }
            // SAFETY: the socket is open and closed exactly once.
            unsafe {
                shutdown(self.socket, SD_BOTH);
                closesocket(self.socket);
            }
            self.socket = INVALID_SOCKET;
        }

        fn channel(&self) -> Option<u8> {
            self.channel
        }
    }

    impl Drop for WinTransport {
        fn drop(&mut self) {
            self.close();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::normalise_address;

    #[test]
    fn bth_addr_parses_any_notation() {
        let expected = Some(0xa8e6_e8fe_d654_u64);
        assert_eq!(bth_addr("A8:E6:E8:FE:D6:54"), expected);
        assert_eq!(bth_addr("a8-e6-e8-fe-d6-54"), expected);
        assert_eq!(bth_addr("A8E6E8FED654"), expected);
    }

    #[test]
    fn bth_addr_rejects_short_or_junk() {
        assert_eq!(bth_addr(""), None);
        assert_eq!(bth_addr("A8:E6:E8:FE:D6"), None);
        assert_eq!(bth_addr("A8:E6:E8:FE:D6:54:00"), None);
        assert_eq!(bth_addr("not an address"), None);
        assert_eq!(bth_addr("ZZ:E6:E8:FE:D6:54"), None);
    }

    #[test]
    fn v2_guid_layout() {
        let guid = v2_guid();
        assert_eq!(guid.data1, 0x956C_7B26);
        assert_eq!(guid.data2, 0xD49A);
        assert_eq!(guid.data3, 0x4BA8);
        assert_eq!(guid.data4, [0xB0, 0x3F, 0xB1, 0x7D, 0x39, 0x3C, 0xB6, 0xE2]);
    }

    #[test]
    fn winsock_error_mapping() {
        assert_eq!(map_recv(5, 0).unwrap(), Some(5));
        assert_eq!(map_recv(SOCKET_ERROR, WSAETIMEDOUT).unwrap(), None);
        let reset = map_recv(SOCKET_ERROR, WSAECONNRESET).unwrap_err();
        assert_eq!(reset.kind(), io::ErrorKind::ConnectionReset);
        let not_conn = map_recv(SOCKET_ERROR, WSAENOTCONN).unwrap_err();
        assert_eq!(not_conn.kind(), io::ErrorKind::NotConnected);
        let closed = map_recv(0, 0).unwrap_err();
        assert_eq!(closed.kind(), io::ErrorKind::UnexpectedEof);
        assert!(map_recv(SOCKET_ERROR, 10050).is_err());
    }

    #[test]
    fn candidate_from_renders_colon_lowercase() {
        let c = candidate_from("ULT WEAR", 0xa8e6_e8fe_d654, true);
        assert_eq!(c.name, "ULT WEAR");
        assert_eq!(c.address, "a8:e6:e8:fe:d6:54");
        assert!(c.connected_audio);
        assert_eq!(
            normalise_address(&c.address),
            normalise_address("A8-E6-E8-FE-D6-54")
        );
        let small = candidate_from("x", 0x0000_0000_0102, false);
        assert_eq!(small.address, "00:00:00:00:01:02");
    }

    #[test]
    fn connect_refuses_unconnected_candidate() {
        let paired = candidate_from("ULT WEAR", bth_addr("A8:E6:E8:FE:D6:54").unwrap(), false);
        assert!(!paired.connected_audio);
        let err = check_connected(&paired).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::NotConnected);
        let connected = candidate_from("ULT WEAR", bth_addr("A8:E6:E8:FE:D6:54").unwrap(), true);
        assert!(check_connected(&connected).is_ok());
    }

    #[cfg(windows)]
    #[test]
    fn wsa_init_is_idempotent() {
        platform::wsa_init().expect("first WSAStartup");
        assert!(platform::wsa_initialised());
        platform::wsa_init().expect("second call reuses the first result");
        platform::wsa_init().expect("and so does a third");
    }
}
