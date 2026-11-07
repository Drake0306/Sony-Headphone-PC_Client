# Phase 3a: Windows RFCOMM Transport — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `mdr-transport` discovers paired Sony headphones and opens the MDR v2 RFCOMM channel on Windows through Winsock, so the same broker, session and UI run there once the tray flyout (Phase 3b, needs a Windows machine) exists.

**Architecture:** A `windows` module in `crates/mdr-transport` mirrors `macos.rs`: `discover()` walks paired Bluetooth devices with `BluetoothFindFirstDevice`/`BluetoothFindNextDevice` and reports connected ones named like the headphones; `connect(address)` opens a `SOCK_STREAM`/`BTHPROTO_RFCOMM` socket to `SOCKADDR_BTH { btAddr, serviceClassId = v2 UUID, port = 0 }` so the stack resolves the channel from SDP, sets `SO_RCVTIMEO` for `recv_timeout`, and reports `channel()` from `getpeername` when the stack fills the port. `discover.rs` dispatches to it under `cfg(windows)`. Everything is unit-tested where no socket is needed and compiled on a Windows CI runner.

**Tech Stack:** `windows-sys` 0.59 (features `Win32_Foundation`, `Win32_Networking_WinSock`, `Win32_Devices_Bluetooth`), `cargo check --target x86_64-pc-windows-msvc` locally, GitHub Actions `windows-latest` job.

**Spec:** `docs/superpowers/specs/2026-11-07-ult-wear-desktop-app-design.md` — Core: "Windows via `windows` crate Winsock `AF_BTH`/`BTHPROTO_RFCOMM` with `serviceClassId`"; Phases: "3. Windows. Winsock transport, tray flyout, same UI. Hardware validation on a Windows machine when Roy has one available; otherwise built and unit-tested only."

## Global Constraints

- No AI attribution; plain commits; every commit passes `cargo test --workspace`, clippy `-D warnings`, `cargo fmt --all -- --check` on macOS AND `cargo check -p mdr-transport --target x86_64-pc-windows-msvc` (and clippy for that target) locally.
- macOS behaviour is unchanged: no file outside `crates/mdr-transport`, `Cargo.lock`, `.github/workflows/ci.yml`, `README.md`, `docs/` changes.
- The transport never sends bytes itself; it only moves what the session gives it. Deny-list unaffected.
- Nothing is claimed as hardware-verified. README and docs say "compiles and passes unit tests on Windows CI; not yet run against the headphones on Windows".

## Review Focus

1. A device paired but not connected must yield `connected_audio: false` and `connect` must fail with `NotConnected`, not hang — pinned by `connect_refuses_unconnected_candidate` (uses the pure `candidate_from` mapping and the address parser).
2. Address notation: `A8:E6:E8:FE:D6:54`, `a8-e6-e8-fe-d6-54` and `A8E6E8FED654` must all produce the same `BTH_ADDR` u64 — `bth_addr_parses_any_notation`.
3. `recv_timeout` must return `Ok(None)` on timeout (WSAETIMEDOUT) and `Err` on `WSAECONNRESET`/`WSAENOTCONN`/zero-length read — `winsock_error_mapping` on the pure mapping function.
4. The v2 UUID must be laid out as a Windows `GUID` exactly (`956C7B26-D49A-4BA8-B03F-B17D393CB6E2` → Data1 0x956C7B26, Data2 0xD49A, Data3 0x4BA8, Data4 [B0,3F,B1,7D,39,3C,B6,E2]) — `v2_guid_layout`.
5. WSAStartup must run once per process and never be followed by WSACleanup while a transport lives — a `OnceLock` guard; `wsa_init_is_idempotent` (Windows-only test).

---

### Task 1: Windows transport module + cross-target CI

**Files:**
- Create: `crates/mdr-transport/src/windows.rs`
- Modify: `crates/mdr-transport/Cargo.toml` (`[target.'cfg(windows)'.dependencies] windows-sys`), `crates/mdr-transport/src/lib.rs` (`#[cfg(windows)] mod windows;`), `crates/mdr-transport/src/discover.rs` (dispatch + `unsupported()` message "only macOS and Windows"), `.github/workflows/ci.yml`, `README.md` (Status: "Phase 3a: Windows transport compiles and is unit-tested on Windows CI; the Windows tray app and hardware validation are next"), `docs/hardware-validation.md` (§7 Windows: to do when a Windows machine is available).
- Test: `crates/mdr-transport/src/windows.rs` unit tests (pure helpers are `pub(crate)` and testable on every OS; socket tests `#[cfg(windows)]`).

**Interfaces:**
- Produces: `pub(crate) fn discover() -> io::Result<Vec<Candidate>>`, `pub(crate) fn connect(address: &str) -> io::Result<Box<dyn Transport>>`, pure helpers `fn bth_addr(address: &str) -> Option<u64>`, `fn v2_guid() -> GUID`-equivalent `[u8; 16]`/struct, `fn map_recv(result: i32, err: i32) -> io::Result<Option<usize>>`, `fn candidate_from(name: &str, address: u64, connected: bool) -> Candidate` (address rendered `aa:bb:cc:dd:ee:ff` lowercase).
- Consumes: `Transport` trait, `Candidate`, `is_ult_wear_name`, `normalise_address` from `lib.rs`.

- [ ] **Step 1: Failing tests** (pure helpers, run on macOS): `bth_addr_parses_any_notation`, `bth_addr_rejects_short_or_junk`, `v2_guid_layout`, `winsock_error_mapping`, `candidate_from_renders_colon_lowercase`, `connect_refuses_unconnected_candidate` (via a pure `check_connected(candidate) -> io::Result<()>`).
- [ ] **Step 2:** `cargo test -p mdr-transport` → compile errors. **Step 3: Implement** the module: pure helpers first, then `WinTransport { socket: SOCKET, channel: Option<u8> }` with `write` (loop until all sent), `recv_timeout` (set `SO_RCVTIMEO` to the timeout in ms, `recv` into a 1024-byte buffer, map via `map_recv`), `close` (`shutdown` + `closesocket`), `Drop` closes; `discover()` via `BluetoothFindFirstDevice` with `BLUETOOTH_DEVICE_SEARCH_PARAMS { fReturnAuthenticated: 1, fReturnConnected: 1, fReturnRemembered: 1, fReturnUnknown: 0, fIssueInquiry: 0 }`, filter `is_ult_wear_name(szName)`, `connected_audio = fConnected != 0`; `connect()` requires the candidate to be connected (NotConnected otherwise), `WSAStartup` through a `OnceLock`, `socket(AF_BTH, SOCK_STREAM, BTHPROTO_RFCOMM)`, `connect()` to `SOCKADDR_BTH`, then `getpeername` to read the port for `channel()`.
- [ ] **Step 4:** green on macOS (`cargo test --workspace`, clippy, fmt) and `cargo check -p mdr-transport --target x86_64-pc-windows-msvc && cargo clippy -p mdr-transport --target x86_64-pc-windows-msvc -- -D warnings`.
- [ ] **Step 5: CI** — add a second job `windows-crates` on `windows-latest`: checkout, stable toolchain, rust-cache, `cargo test -p mdr-frame -p mdr-proto -p mdr-session -p mdr-transport`, `cargo clippy -p mdr-transport --all-targets -- -D warnings`. The Tauri app is NOT built on Windows yet.
- [ ] **Step 6: Commit** `Add the Windows RFCOMM transport` (code) and `Check the protocol crates on Windows in CI` (ci + docs).

---

### Task 2 (Phase 3b, not in this run): Windows tray flyout and app build — needs a Windows machine to see; plan separately.
