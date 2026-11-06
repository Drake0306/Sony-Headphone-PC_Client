# Phase 1: MDR core crates and macOS popover — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A Rust protocol library (`mdr-frame`, `mdr-proto`, `mdr-session`, `mdr-transport`) and a Tauri menu-bar app whose popover shows live battery, noise control, ambient level, ULT level and the playing device for a WH-ULT900N over Bluetooth on macOS.

**Architecture:** Sans-IO layering: `mdr-frame` turns bytes into frames and back; `mdr-proto` turns frame payloads into typed messages and builds requests; `mdr-session` is a state machine that owns the handshake, ARQ, capability gating, deny-list and a `DeviceState` with per-field confirmation status; `mdr-transport` moves bytes over RFCOMM (IOBluetooth on macOS) or a scripted fake. The Tauri app runs one broker thread that owns the session and transport and emits state to a Svelte popover.

**Tech Stack:** Rust 1.93 (edition 2021), Cargo workspace; `objc2-io-bluetooth 0.3`, `objc2-foundation 0.3`, `dispatch2`; Tauri 2.12, `tauri-nspanel 2.1`, `tauri-plugin-positioner 2.4`; Svelte 5.57, Vite 8, TypeScript; fonts Instrument Sans, Doto, DM Mono (OFL).

**Spec:** `docs/superpowers/specs/2026-11-07-ult-wear-desktop-app-design.md` (read it first; `docs/protocol-notes.md` carries the byte layouts every task below cites).

## Global Constraints

- No AI attribution anywhere: commit messages have no trailers; see `CLAUDE.md`. Commit as the repo's configured identity (`git config user.name` = Drake0306).
- Product codename **Hairline**; never "Sony" or "Sound Connect" in product names or bundle identifiers. Bundle id `com.drake0306.hairline`.
- Deny-list from `docs/protocol-notes.md` is enforced in `mdr-session` and never bypassed by the app in Phase 1 (no Advanced panel yet).
- Every write is gated: the capability must be declared by `06/07` or marked proven for this unit; otherwise the request returns `SessionError::Unavailable`.
- Read-modify-write for EQ/ULT: a `58 03` SET always echoes the last RET's preset and bands, changing only the ULT byte.
- Service UUID `956C7B26-D49A-4BA8-B03F-B17D393CB6E2`; RFCOMM channel from SDP, never hard-coded.
- Popover is 340 px wide; tokens: paper `#efede6`, ink `#161614`, forest `#4a5550`, hairline `#cfccc2`, muted `#6f726d`, ULT orange `#ff6600`, box `#f8f7f3`; fonts bundled, never fetched.
- Rust: `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test --workspace` pass at every commit. Frontend: `pnpm check` passes.

## Review Focus

1. A frame split across two `read()` calls (header in one, tail in the next) must decode once, not twice and not never — pinned in Task 1 (`decodes_frame_split_across_pushes`).
2. A frame whose checksum byte itself is `0x3C`/`0x3D`/`0x3E` must round-trip (escape applies to the checksum) — pinned in Task 1 (`escapes_checksum_byte`).
3. The device's unsolicited NTFY (`69 17 …` when the user presses the cup button) must update `DeviceState` with status Confirmed even with no request pending — pinned in Task 3 (`ntfy_without_request_updates_state`).
4. A SET that gets an ACK but no RET/NTFY within 1.5 s must flip to Refused("no reply") and trigger a GET — pinned in Task 3 (`set_without_reply_becomes_refused_and_requeries`).
5. Losing the RFCOMM link mid-session (transport read returns `Ok(0)`/error) must put every field into Unavailable and the popover into "Not connected" without a crash — pinned in Task 5 (`disconnect_marks_all_unavailable`).

---

### Task 1: Workspace and `mdr-frame`

**Files:**
- Create: `Cargo.toml` (workspace), `rust-toolchain.toml`, `crates/mdr-frame/Cargo.toml`, `crates/mdr-frame/src/lib.rs`, `crates/mdr-frame/src/codec.rs`, `crates/mdr-frame/src/decoder.rs`
- Test: `crates/mdr-frame/tests/vectors.rs`

**Interfaces:**
- Produces:
  - `pub enum FrameType { Ack, Command1, Command2, Other(u8) }` with `fn to_u8(self) -> u8` (`0x01`, `0x0C`, `0x0E`) and `fn from_u8(u8) -> Self`.
  - `pub struct Frame { pub frame_type: FrameType, pub seq: u8, pub payload: Vec<u8> }`
  - `pub fn encode(frame: &Frame) -> Vec<u8>`
  - `pub fn ack_for(received_seq: u8) -> Frame` (type Ack, seq `1 - received_seq`, empty payload)
  - `pub struct Decoder { .. }` with `Decoder::new()`, `fn push(&mut self, bytes: &[u8]) -> Vec<Result<Frame, FrameError>>`
  - `pub enum FrameError { BadChecksum { expected: u8, got: u8 }, BadLength, Truncated }`

- [ ] **Step 1: Create the workspace**

`Cargo.toml` at the root with `[workspace] members = ["crates/*", "apps/desktop/src-tauri"] resolver = "2"` and `[workspace.package] edition = "2021" license = "MIT" rust-version = "1.85"`. `rust-toolchain.toml` pins `channel = "stable"`. Create `crates/mdr-frame` with `cargo new --lib`. Set `[lib] name = "mdr_frame"`.

- [ ] **Step 2: Write the failing vector tests**

`crates/mdr-frame/tests/vectors.rs`:

```rust
use mdr_frame::{ack_for, encode, Decoder, Frame, FrameType};

#[test]
fn encodes_protocol_info_request() {
    let f = Frame { frame_type: FrameType::Command1, seq: 0, payload: vec![0x00, 0x00] };
    assert_eq!(encode(&f), [0x3E, 0x0C, 0x00, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x0E, 0x3C]);
}

#[test]
fn encodes_ack_with_flipped_seq() {
    assert_eq!(encode(&ack_for(0)), [0x3E, 0x01, 0x01, 0x00, 0x00, 0x00, 0x00, 0x02, 0x3C]);
    assert_eq!(encode(&ack_for(1)), [0x3E, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x3C]);
}

#[test]
fn escapes_payload_bytes() {
    let f = Frame { frame_type: FrameType::Command1, seq: 1, payload: vec![0x3E, 0x3C, 0x3D] };
    let bytes = encode(&f);
    // payload 3E 3C 3D -> 3D 2E, 3D 2C, 3D 2D ; checksum = 0C+01+03+3E+3C+3D = 0xC7
    assert_eq!(bytes, [0x3E, 0x0C, 0x01, 0x00, 0x00, 0x00, 0x03, 0x3D, 0x2E, 0x3D, 0x2C, 0x3D, 0x2D, 0xC7, 0x3C]);
    let mut d = Decoder::new();
    let out = d.push(&bytes);
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].as_ref().unwrap().payload, vec![0x3E, 0x3C, 0x3D]);
}

#[test]
fn escapes_checksum_byte() {
    // choose payload so checksum == 0x3E: 0C + 00 + 01 + x = 0x3E -> x = 0x31
    let f = Frame { frame_type: FrameType::Command1, seq: 0, payload: vec![0x31] };
    let bytes = encode(&f);
    assert_eq!(&bytes[bytes.len() - 3..], [0x3D, 0x2E, 0x3C]);
    let mut d = Decoder::new();
    assert_eq!(d.push(&bytes)[0].as_ref().unwrap().payload, vec![0x31]);
}

#[test]
fn decodes_battery_reply() {
    // 23 00 43 00 from the device: type 0C seq 01 len 4 ; checksum = 0C+01+04+23+00+43+00 = 0x77
    let bytes = [0x3E, 0x0C, 0x01, 0x00, 0x00, 0x00, 0x04, 0x23, 0x00, 0x43, 0x00, 0x77, 0x3C];
    let mut d = Decoder::new();
    let out = d.push(&bytes);
    let f = out[0].as_ref().unwrap();
    assert_eq!(f.frame_type, FrameType::Command1);
    assert_eq!(f.seq, 1);
    assert_eq!(f.payload, vec![0x23, 0x00, 0x43, 0x00]);
}

#[test]
fn decodes_frame_split_across_pushes() {
    let bytes = [0x3E, 0x0C, 0x01, 0x00, 0x00, 0x00, 0x04, 0x23, 0x00, 0x43, 0x00, 0x77, 0x3C];
    let mut d = Decoder::new();
    assert!(d.push(&bytes[..6]).is_empty());
    let out = d.push(&bytes[6..]);
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].as_ref().unwrap().payload, vec![0x23, 0x00, 0x43, 0x00]);
}

#[test]
fn reports_bad_checksum_and_resyncs() {
    let mut bad = vec![0x3E, 0x0C, 0x01, 0x00, 0x00, 0x00, 0x04, 0x23, 0x00, 0x43, 0x00, 0x00, 0x3C];
    bad.extend_from_slice(&[0x3E, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x3C]);
    let mut d = Decoder::new();
    let out = d.push(&bad);
    assert_eq!(out.len(), 2);
    assert!(matches!(out[0], Err(mdr_frame::FrameError::BadChecksum { expected: 0x77, got: 0x00 })));
    assert_eq!(out[1].as_ref().unwrap().frame_type, FrameType::Ack);
}
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo test -p mdr-frame`
Expected: compile error, `encode` not found.

- [ ] **Step 4: Implement `codec.rs` (`encode`, `ack_for`, checksum) and `decoder.rs` (`Decoder`)**

Checksum = wrapping sum of type, seq, four length bytes and payload. Escaping applies to everything between the `0x3E` start and the `0x3C` end, including the checksum. The decoder is a byte-at-a-time state machine (Idle → InFrame, with an `escape_next` flag) that buffers unescaped inner bytes until `0x3C`, then validates length (`len == inner.len() - 7`) and checksum. On error it returns the error and resets to Idle; bytes outside a frame are dropped.

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p mdr-frame && cargo clippy -p mdr-frame --all-targets -- -D warnings`
Expected: 7 passed, clippy clean.

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml rust-toolchain.toml crates/mdr-frame
git commit -m "Add workspace and mdr-frame codec with verified vectors"
```

---

### Task 2: `mdr-proto` — opcodes, messages, request builders, deny-list

**Files:**
- Create: `crates/mdr-proto/Cargo.toml`, `src/lib.rs`, `src/opcode.rs`, `src/types.rs`, `src/message.rs`, `src/request.rs`, `src/deny.rs`
- Test: `crates/mdr-proto/tests/decode.rs`, `crates/mdr-proto/tests/requests.rs`

**Interfaces:**
- Consumes: nothing from other crates.
- Produces (all `pub`, all `Clone + Debug + PartialEq`, serde `Serialize`/`Deserialize` on state types; `Copy` on `NcMode`, `NcAsmState`, `UltLevel`, `Codec`, `Charging`, `Table`, `Kind`, `Group`):
  - `enum Table { T1, T2 }`
  - `struct Opcode(pub u8)` with consts in `opcode::t1` (`CONNECT_GET_PROTOCOL_INFO = 0x00` … exactly the table in `docs/protocol-notes.md`) and `opcode::t2`.
  - `enum NcMode { Off, NoiseCancelling, Ambient }`
  - `struct NcAsmState { pub mode: NcMode, pub level: u8 /*1..=20*/, pub voice_focus: bool }`
  - `enum UltLevel { Off = 0, Ult1 = 1, Ult2 = 2 }`
  - `struct EqUltState { pub preset: u8, pub ult: UltLevel, pub bands: Vec<u8> }`
  - `struct EqUltCapability { pub bands: u8, pub steps: u8, pub ult2_additional_step: u8, pub presets: Vec<(u8, String)> }`
  - `enum Codec { Unsettled, Sbc, Aac, Ldac, Lc3, Other(u8) }`
  - `enum Charging { NotCharging, Charging, Unknown, Charged }`
  - `enum DeviceInfo { ModelName(String), Firmware(String), SeriesAndColor { series: u8, color: u8 } }`
  - `struct SupportFunction { pub id: u8, pub priority: u8 }`
  - `struct GsCapability { pub setting: u8, pub kind: GsKind, pub subject: String, pub summary: String, pub elements: Vec<(String, String)> }` with `enum GsKind { Boolean, List }`
  - `enum Message { ProtocolInfo { version: u32, table1: bool, table2: bool }, SupportFunctions(Vec<SupportFunction>), DeviceInfo(DeviceInfo), Battery { level: u8, charging: Charging }, NcAsm(NcAsmState), EqUlt(EqUltState), EqUltCapability(EqUltCapability), Codec(Codec), Volume(u8), GsCapability(GsCapability), GsBool { setting: u8, on: bool }, GsList { setting: u8, index: u8 }, AutoPowerOff { current: u8, last: u8 }, Unknown { table: Table, opcode: u8, payload: Vec<u8> } }`
  - `enum Kind { Ret, Ntfy, Other }` and `fn decode(table: Table, payload: &[u8]) -> (Kind, Message)` — never panics; short or unknown payloads yield `Message::Unknown`.
  - `struct Request { pub table: Table, pub bytes: Vec<u8>, pub capability: Option<u8>, pub group: Group }` where `enum Group { Connect, Battery, NcAsm, EqUlt, Codec, Volume, GeneralSetting(u8), AutoPowerOff, Power }`.
  - `mod request` functions, each returning `Request`: `protocol_info()`, `support_functions(table: Table)`, `device_info(kind: u8)`, `battery_get()`, `ncasm_get()`, `ncasm_set(&NcAsmState)`, `eq_ult_capability()`, `eq_ult_get()`, `eq_ult_set(&EqUltState)`, `codec_get()`, `volume_get()`, `volume_set(u8)`, `gs_capability(setting: u8)`, `gs_get(setting: u8)`, `gs_set_bool(setting: u8, on: bool)`, `gs_set_list(setting: u8, index: u8)`, `auto_power_off_get()`, `auto_power_off_set(current: u8)`, `power_off()`.
  - `fn denied(table: Table, bytes: &[u8]) -> Option<&'static str>` (reason if on the deny-list).

- [ ] **Step 1: Write the failing decode tests**

`tests/decode.rs` (values from `docs/protocol-notes.md`):

```rust
use mdr_proto::*;

#[test]
fn protocol_info_v2() {
    let (k, m) = decode(Table::T1, &[0x01, 0x00, 0x03, 0x00, 0x20, 0x02, 0x00, 0x00]);
    assert_eq!(k, Kind::Ret);
    assert_eq!(m, Message::ProtocolInfo { version: 0x0300_2002, table1: true, table2: true });
}

#[test]
fn support_functions_list() {
    let (_, m) = decode(Table::T1, &[0x07, 0x00, 0x02, 0x20, 0x01, 0x53, 0x01]);
    assert_eq!(m, Message::SupportFunctions(vec![SupportFunction { id: 0x20, priority: 1 }, SupportFunction { id: 0x53, priority: 1 }]));
}

#[test]
fn battery_67_percent() {
    let (k, m) = decode(Table::T1, &[0x23, 0x00, 0x43, 0x00]);
    assert_eq!(k, Kind::Ret);
    assert_eq!(m, Message::Battery { level: 67, charging: Charging::NotCharging });
    let (k2, _) = decode(Table::T1, &[0x25, 0x00, 0x43, 0x01]);
    assert_eq!(k2, Kind::Ntfy);
}

#[test]
fn ncasm_ambient_voice_level_14() {
    let (_, m) = decode(Table::T1, &[0x67, 0x17, 0x01, 0x01, 0x01, 0x01, 0x0E]);
    assert_eq!(m, Message::NcAsm(NcAsmState { mode: NcMode::Ambient, level: 14, voice_focus: true }));
    let (k, m) = decode(Table::T1, &[0x69, 0x17, 0x01, 0x00, 0x00, 0x00, 0x0E]);
    assert_eq!(k, Kind::Ntfy);
    assert_eq!(m, Message::NcAsm(NcAsmState { mode: NcMode::Off, level: 14, voice_focus: false }));
}

#[test]
fn eq_ult_reply() {
    let (_, m) = decode(Table::T1, &[0x57, 0x03, 0x00, 0x02, 0x06, 0x0A, 0x0A, 0x0A, 0x0A, 0x0A, 0x0A]);
    assert_eq!(m, Message::EqUlt(EqUltState { preset: 0x00, ult: UltLevel::Ult2, bands: vec![10; 6] }));
}

#[test]
fn eq_ult_capability_with_presets() {
    let p = [0x51, 0x03, 0x06, 0x15, 0x0A, 0x02, 0x00, 0x03, b'O', b'f', b'f', 0x16, 0x04, b'B', b'a', b's', b's'];
    let (_, m) = decode(Table::T1, &p);
    assert_eq!(m, Message::EqUltCapability(EqUltCapability { bands: 6, steps: 21, ult2_additional_step: 10, presets: vec![(0x00, "Off".into()), (0x16, "Bass".into())] }));
}

#[test]
fn codec_ldac_and_model_name() {
    assert_eq!(decode(Table::T1, &[0x13, 0x02, 0x10]).1, Message::Codec(Codec::Ldac));
    let name = [0x05, 0x01, 0x08, b'U', b'L', b'T', b' ', b'W', b'E', b'A', b'R'];
    assert_eq!(decode(Table::T1, &name).1, Message::DeviceInfo(DeviceInfo::ModelName("ULT WEAR".into())));
}

#[test]
fn gs_bool_inverted_enum() {
    assert_eq!(decode(Table::T1, &[0xD7, 0xD1, 0x00, 0x00]).1, Message::GsBool { setting: 0xD1, on: true });
    assert_eq!(decode(Table::T1, &[0xD7, 0xD1, 0x00, 0x01]).1, Message::GsBool { setting: 0xD1, on: false });
}

#[test]
fn short_and_unknown_payloads_do_not_panic() {
    assert!(matches!(decode(Table::T1, &[0x23]).1, Message::Unknown { .. }));
    assert!(matches!(decode(Table::T2, &[0x47, 0x20, 0xFF]).1, Message::Unknown { .. }));
    assert!(matches!(decode(Table::T1, &[]).1, Message::Unknown { .. }));
}
```

- [ ] **Step 2: Write the failing request tests**

`tests/requests.rs`:

```rust
use mdr_proto::*;

#[test]
fn builders_produce_documented_bytes() {
    assert_eq!(request::protocol_info().bytes, [0x00, 0x00]);
    assert_eq!(request::support_functions(Table::T2).table, Table::T2);
    assert_eq!(request::battery_get().bytes, [0x22, 0x00]);
    assert_eq!(request::ncasm_get().bytes, [0x66, 0x17]);
    assert_eq!(request::ncasm_set(&NcAsmState { mode: NcMode::Ambient, level: 14, voice_focus: true }).bytes, [0x68, 0x17, 0x01, 0x01, 0x01, 0x01, 0x0E]);
    assert_eq!(request::ncasm_set(&NcAsmState { mode: NcMode::NoiseCancelling, level: 14, voice_focus: true }).bytes, [0x68, 0x17, 0x01, 0x01, 0x00, 0x01, 0x0E]);
    assert_eq!(request::ncasm_set(&NcAsmState { mode: NcMode::Off, level: 14, voice_focus: false }).bytes, [0x68, 0x17, 0x01, 0x00, 0x00, 0x00, 0x0E]);
    assert_eq!(request::eq_ult_set(&EqUltState { preset: 0, ult: UltLevel::Ult1, bands: vec![10; 6] }).bytes, [0x58, 0x03, 0x00, 0x01, 0x06, 0x0A, 0x0A, 0x0A, 0x0A, 0x0A, 0x0A]);
    assert_eq!(request::volume_set(12).bytes, [0xA8, 0x20, 0x0C]);
    assert_eq!(request::gs_set_bool(0xD1, true).bytes, [0xD8, 0xD1, 0x00, 0x00]);
    assert_eq!(request::power_off().bytes, [0x24, 0x03, 0x01]);
}

#[test]
fn requests_declare_their_capability_and_group() {
    assert_eq!(request::ncasm_set(&NcAsmState { mode: NcMode::Off, level: 1, voice_focus: false }).capability, Some(0x6B));
    assert_eq!(request::eq_ult_get().capability, Some(0x53));
    assert_eq!(request::battery_get().capability, Some(0x20));
    assert_eq!(request::protocol_info().capability, None);
    assert_eq!(request::eq_ult_get().group, Group::EqUlt);
}

#[test]
fn deny_list_is_keyed_on_opcode_and_sub_id() {
    assert!(denied(Table::T1, &[0xF8, 0x09, 0x01]).is_some());
    assert!(denied(Table::T1, &[0x98, 0x00, 0x16, 0x01]).is_some());
    assert!(denied(Table::T1, &[0x98, 0x00, 0x06, 0x01]).is_none());
    assert!(denied(Table::T1, &[0xC4, 0x00]).is_some());
    assert!(denied(Table::T1, &[0x3E, 0x00]).is_some());
    assert!(denied(Table::T1, &[0x24, 0x03, 0x01]).is_some()); // power off needs session-level confirmation
    assert!(denied(Table::T1, &[0x68, 0x17, 0x01, 0x01, 0x01, 0x00, 0x05]).is_none());
}
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo test -p mdr-proto`
Expected: compile errors (types missing).

- [ ] **Step 4: Implement `opcode.rs`, `types.rs`, `message.rs`, `request.rs`, `deny.rs`**

`decode` dispatches on `(table, payload[0], payload[1])`; `Kind` is Ret for `x1`/`x3`/`x7`/`xB` low nibbles, Ntfy for `x5`/`x9`/`xD`. Strings are `u8 len + bytes` (lossy UTF-8). `denied` entries: `(T1, 0xF8, Some(0x09))`, `(T1, 0x98, Some(0x16))`, `(T1, 0x24, None)`, `(T1, 0xC4, None)`, `(T1, 0x30..=0x3F, None)`, `(T1, 0x2D, None)`; a sub-id of `None` matches any. Confirmed-action opcodes (`0x24`) are on the list; `mdr-session` lifts them only via `Session::request_confirmed`.

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p mdr-proto && cargo clippy -p mdr-proto --all-targets -- -D warnings`
Expected: all pass, clippy clean.

- [ ] **Step 6: Commit**

```bash
git add crates/mdr-proto
git commit -m "Add mdr-proto message codecs, request builders and deny-list"
```

---

### Task 3: `mdr-session` — sans-IO session with ARQ, handshake, gating and state

**Files:**
- Create: `crates/mdr-session/Cargo.toml`, `src/lib.rs`, `src/session.rs`, `src/arq.rs`, `src/state.rs`, `src/handshake.rs`
- Test: `crates/mdr-session/tests/conversation.rs`, `crates/mdr-session/tests/gating.rs`

**Interfaces:**
- Consumes: `mdr_frame::{encode, Decoder, Frame, FrameType, ack_for}`, `mdr_proto::*`.
- Produces:
  - `enum Status { Confirmed, Pending { since_ms: u64 }, Refused { reason: String }, Unavailable }`
  - `struct Tracked<T> { pub value: Option<T>, pub status: Status }`
  - `struct PairedDevice { pub name: String, pub playing: bool }` (host-side, filled by the app)
  - `struct DeviceState { pub connected: bool, pub model: Option<String>, pub firmware: Option<String>, pub color: Option<u8>, pub capabilities_t1: Vec<u8>, pub capabilities_t2: Vec<u8>, pub proven: Vec<(Table, u8)>, pub battery: Tracked<(u8, Charging)>, pub nc: Tracked<NcAsmState>, pub eq_ult: Tracked<EqUltState>, pub eq_capability: Option<EqUltCapability>, pub codec: Tracked<Codec>, pub volume: Tracked<u8>, pub multipoint: Tracked<bool>, pub auto_power_off: Tracked<u8>, pub gs: Vec<GsCapability>, pub devices: Vec<PairedDevice> }` (serde Serialize)
  - `enum Event { StateChanged, Connected, Disconnected, Log(String) }`
  - `enum SessionError { Unavailable(u8), Denied(&'static str), NotConnected, Busy }`
  - `struct Session { .. }` with:
    - `fn new(now_ms: u64) -> Session`
    - `fn start(&mut self)` — queues the handshake (Task 3 Step 4 lists the order)
    - `fn handle_input(&mut self, bytes: &[u8], now_ms: u64) -> Vec<Event>`
    - `fn poll_output(&mut self) -> Option<Vec<u8>>` — next bytes to write, respecting one-data-frame-in-flight
    - `fn handle_timeout(&mut self, now_ms: u64) -> Vec<Event>`
    - `fn next_timeout_ms(&self) -> Option<u64>`
    - `fn request(&mut self, req: Request) -> Result<(), SessionError>` — gating + deny-list, marks the group Pending
    - `fn request_confirmed(&mut self, req: Request) -> Result<(), SessionError>` — same but lifts `0x24` power off
    - `fn set_nc(&mut self, s: NcAsmState) -> Result<(), SessionError>`, `fn set_ult(&mut self, ult: UltLevel) -> Result<(), SessionError>` (read-modify-write from `state.eq_ult.value`; `Err(Busy)` if no EQ value yet)
    - `fn state(&self) -> &DeviceState`
    - `fn mark_disconnected(&mut self) -> Vec<Event>`
    - `fn mark_proven(&mut self, table: Table, function: u8)`
    - `fn set_devices(&mut self, devices: Vec<PairedDevice>)`
  - Constants: `ACK_TIMEOUT_MS = 1000`, `ACK_RETRIES = 3`, `REPLY_TIMEOUT_MS = 1500`.

- [ ] **Step 1: Write the failing conversation tests**

`tests/conversation.rs` and `tests/gating.rs` share `tests/common/mod.rs` with helpers: `fn frame(t: FrameType, seq: u8, p: &[u8]) -> Vec<u8>` (wraps `mdr_frame::encode`), `fn drain(s: &mut Session) -> Vec<Vec<u8>>` (collects `poll_output` until None), `fn decode_payload(bytes: &[u8]) -> Vec<u8>` and `fn seq_of(bytes: &[u8]) -> u8` (via `mdr_frame::Decoder`), `fn connected_session() -> Session` (runs the full script below), `fn connected_session_with_caps(t1: &[u8]) -> Session` (same script but the `07 00` reply lists only the given functions, and the handshake skips reads whose capability is missing):

```rust
#[test]
fn handshake_sends_protocol_info_first_and_waits_for_ack() {
    let mut s = Session::new(0);
    s.start();
    let out = drain(&mut s);
    assert_eq!(out, vec![frame(FrameType::Command1, 0, &[0x00, 0x00])]);
    assert!(s.poll_output().is_none(), "one frame in flight");
    s.handle_input(&frame(FrameType::Ack, 1, &[]), 10);
    s.handle_input(&frame(FrameType::Command1, 0, &[0x01, 0x00, 0x03, 0x00, 0x20, 0x02, 0x00, 0x00]), 11);
    let out = drain(&mut s);
    assert_eq!(out[0], frame(FrameType::Ack, 1, &[]), "device data frames are ACKed immediately");
    assert_eq!(out[1], frame(FrameType::Command1, 1, &[0x06, 0x00]));
}

#[test]
fn full_handshake_reaches_connected_and_reads_initial_state() {
    let mut s = Session::new(0);
    s.start();
    let script: Vec<(&[u8], &[u8])> = vec![ // (expected request, device reply) in order
        (&[0x00, 0x00], &[0x01, 0x00, 0x03, 0x00, 0x20, 0x02, 0x00, 0x00]),
        (&[0x06, 0x00], &[0x07, 0x00, 0x05, 0x20, 0x01, 0x53, 0x01, 0x6B, 0x01, 0x12, 0x01, 0xD1, 0x01]),
        // T2 support functions
        (&[0x06, 0x00], &[0x07, 0x00, 0x01, 0x42, 0x01]),
        (&[0x04, 0x01], &[0x05, 0x01, 0x08, b'U', b'L', b'T', b' ', b'W', b'E', b'A', b'R']),
        (&[0x04, 0x02], &[0x05, 0x02, 0x05, b'2', b'.', b'1', b'.', b'0']),
        (&[0x22, 0x00], &[0x23, 0x00, 0x43, 0x00]),
        (&[0x66, 0x17], &[0x67, 0x17, 0x01, 0x01, 0x01, 0x01, 0x0E]),
        (&[0x50, 0x03], &[0x51, 0x03, 0x06, 0x15, 0x0A, 0x00]),
        (&[0x56, 0x03], &[0x57, 0x03, 0x00, 0x01, 0x06, 0x0A, 0x0A, 0x0A, 0x0A, 0x0A, 0x0A]),
        (&[0x12, 0x02], &[0x13, 0x02, 0x10]),
        (&[0xD0, 0xD1], &[0xD1, 0xD1, 0x00, 0x00, 0x0A, b'M', b'U', b'L', b'T', b'I', b'P', b'O', b'I', b'N', b'T', 0x00]),
        (&[0xD6, 0xD1], &[0xD7, 0xD1, 0x00, 0x00]),
    ];
    let mut events = vec![];
    let mut t = 0;
    for (i, (expect, reply)) in script.iter().enumerate() {
        let out = drain(&mut s);
        let req = out.last().unwrap();
        assert_eq!(&decode_payload(req), expect, "request #{i}");
        let seq = seq_of(req);
        events.extend(s.handle_input(&frame(FrameType::Ack, 1 - seq, &[]), t));
        let table = if i == 2 { FrameType::Command2 } else { FrameType::Command1 };
        events.extend(s.handle_input(&frame(table, seq, reply), t + 1));
        t += 10;
    }
    assert!(events.contains(&Event::Connected));
    let st = s.state();
    assert!(st.connected);
    assert_eq!(st.model.as_deref(), Some("ULT WEAR"));
    assert_eq!(st.firmware.as_deref(), Some("2.1.0"));
    assert_eq!(st.capabilities_t1, vec![0x20, 0x53, 0x6B, 0x12, 0xD1]);
    assert_eq!(st.battery.value, Some((67, Charging::NotCharging)));
    assert_eq!(st.nc.value, Some(NcAsmState { mode: NcMode::Ambient, level: 14, voice_focus: true }));
    assert_eq!(st.eq_ult.value.as_ref().unwrap().ult, UltLevel::Ult1);
    assert_eq!(st.codec.value, Some(Codec::Ldac));
    assert_eq!(st.multipoint.value, Some(true));
    assert!(matches!(st.battery.status, Status::Confirmed));
}

#[test]
fn ntfy_without_request_updates_state() {
    let mut s = connected_session(); // helper: runs the script above
    let ev = s.handle_input(&frame(FrameType::Command1, 0, &[0x69, 0x17, 0x01, 0x01, 0x00, 0x00, 0x0E]), 500);
    assert!(ev.contains(&Event::StateChanged));
    assert_eq!(s.state().nc.value.unwrap().mode, NcMode::NoiseCancelling);
    assert!(matches!(s.state().nc.status, Status::Confirmed));
    assert_eq!(drain(&mut s)[0], frame(FrameType::Ack, 1, &[]));
}

#[test]
fn set_nc_marks_pending_then_confirmed_on_ntfy() {
    let mut s = connected_session();
    s.set_nc(NcAsmState { mode: NcMode::Off, level: 14, voice_focus: true }).unwrap();
    assert!(matches!(s.state().nc.status, Status::Pending { .. }));
    let req = drain(&mut s).pop().unwrap();
    assert_eq!(decode_payload(&req), [0x68, 0x17, 0x01, 0x00, 0x00, 0x01, 0x0E]);
    let seq = seq_of(&req);
    s.handle_input(&frame(FrameType::Ack, 1 - seq, &[]), 600);
    s.handle_input(&frame(FrameType::Command1, seq, &[0x69, 0x17, 0x01, 0x00, 0x00, 0x01, 0x0E]), 601);
    assert!(matches!(s.state().nc.status, Status::Confirmed));
    assert_eq!(s.state().nc.value.unwrap().mode, NcMode::Off);
}

#[test]
fn set_ult_is_read_modify_write() {
    let mut s = connected_session();
    s.set_ult(UltLevel::Ult2).unwrap();
    let req = drain(&mut s).pop().unwrap();
    assert_eq!(decode_payload(&req), [0x58, 0x03, 0x00, 0x02, 0x06, 0x0A, 0x0A, 0x0A, 0x0A, 0x0A, 0x0A]);
}

#[test]
fn set_without_reply_becomes_refused_and_requeries() {
    let mut s = connected_session();
    s.set_ult(UltLevel::Off).unwrap();
    let req = drain(&mut s).pop().unwrap();
    let seq = seq_of(&req);
    s.handle_input(&frame(FrameType::Ack, 1 - seq, &[]), 1000);
    assert_eq!(s.next_timeout_ms(), Some(1000 + REPLY_TIMEOUT_MS));
    let ev = s.handle_timeout(2500);
    assert!(ev.contains(&Event::StateChanged));
    assert!(matches!(&s.state().eq_ult.status, Status::Refused { reason } if reason == "no reply"));
    assert_eq!(decode_payload(&drain(&mut s).pop().unwrap()), [0x56, 0x03]);
}

#[test]
fn unacked_frame_is_retransmitted_then_gives_up() {
    let mut s = Session::new(0);
    s.start();
    let first = drain(&mut s).pop().unwrap();
    for n in 1..=ACK_RETRIES {
        let ev = s.handle_timeout(n * ACK_TIMEOUT_MS);
        if n < ACK_RETRIES { assert_eq!(drain(&mut s).pop().unwrap(), first); } else { assert!(ev.contains(&Event::Disconnected)); }
    }
}

#[test]
fn disconnect_marks_all_unavailable() {
    let mut s = connected_session();
    let ev = s.mark_disconnected();
    assert!(ev.contains(&Event::Disconnected));
    assert!(!s.state().connected);
    assert!(matches!(s.state().battery.status, Status::Unavailable));
    assert!(matches!(s.state().nc.status, Status::Unavailable));
}
```

- [ ] **Step 2: Write the failing gating tests**

`tests/gating.rs`:

```rust
#[test]
fn undeclared_capability_is_unavailable() {
    let mut s = connected_session_with_caps(&[0x20]); // battery only
    assert_eq!(s.set_nc(NcAsmState { mode: NcMode::Off, level: 1, voice_focus: false }), Err(SessionError::Unavailable(0x6B)));
    assert!(matches!(s.state().nc.status, Status::Unavailable));
}

#[test]
fn proven_capability_is_allowed() {
    let mut s = connected_session_with_caps(&[0x20]);
    s.mark_proven(Table::T1, 0x6B);
    assert!(s.set_nc(NcAsmState { mode: NcMode::Off, level: 1, voice_focus: false }).is_ok());
}

#[test]
fn deny_list_blocks_and_confirmed_path_lifts_power_off() {
    let mut s = connected_session();
    assert!(matches!(s.request(request::power_off()), Err(SessionError::Denied(_))));
    assert!(s.request_confirmed(request::power_off()).is_ok());
    let mut s2 = connected_session();
    let reboot = Request { table: Table::T1, bytes: vec![0x98, 0x00, 0x16, 0x01], capability: None, group: Group::Power };
    assert!(matches!(s2.request_confirmed(reboot), Err(SessionError::Denied(_))));
}

#[test]
fn requests_before_connected_are_rejected() {
    let mut s = Session::new(0);
    assert_eq!(s.request(request::battery_get()), Err(SessionError::NotConnected));
}
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo test -p mdr-session`
Expected: compile errors.

- [ ] **Step 4: Implement `arq.rs`, `handshake.rs`, `state.rs`, `session.rs`**

ARQ: outgoing queue of `(Request, group)`; `in_flight: Option<{frame, sent_at, retries}>`; our data-frame seq alternates 0/1 per sent data frame; incoming data frames are ACKed with `ack_for(seq)` placed at the front of the output queue; duplicate incoming frames (same seq as last) are ACKed but not re-dispatched. Handshake order is exactly the script in Step 1: protocol info, T1 support functions, T2 support functions, model name, firmware, battery, ncasm, eq capability, eq ult, codec, then for each declared `D1..D3`: capability then param; `Connected` is emitted after the last handshake reply. Gating: `request()` checks `denied()` first, then `capability` against `capabilities_tX ∪ proven` (requests with `capability: None` always pass). Pending bookkeeping per `Group`; a RET/NTFY of the same group clears Pending → Confirmed and stores the value; `handle_timeout` turns Pending older than `REPLY_TIMEOUT_MS` into Refused("no reply") and queues the group's GET. `mark_disconnected` sets every `Tracked` to Unavailable and `connected = false`.

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p mdr-session && cargo clippy -p mdr-session --all-targets -- -D warnings`
Expected: all pass, clippy clean.

- [ ] **Step 6: Commit**

```bash
git add crates/mdr-session
git commit -m "Add mdr-session state machine with ARQ, handshake and gating"
```

---

### Task 4: `mdr-transport` — transport trait, fake device, macOS IOBluetooth RFCOMM

**Files:**
- Create: `crates/mdr-transport/Cargo.toml`, `src/lib.rs`, `src/fake.rs`, `src/macos.rs` (cfg macos), `src/discover.rs`
- Test: `crates/mdr-transport/tests/fake.rs`

**Interfaces:**
- Consumes: `mdr_frame` (fake device only), `mdr_proto` (fake device only).
- Produces:
  - `pub const V2_SERVICE_UUID: &str = "956C7B26-D49A-4BA8-B03F-B17D393CB6E2"`
  - `pub struct Candidate { pub name: String, pub address: String, pub connected_audio: bool }`
  - `pub trait Transport: Send { fn write(&mut self, bytes: &[u8]) -> std::io::Result<()>; fn recv_timeout(&mut self, timeout: std::time::Duration) -> std::io::Result<Option<Vec<u8>>>; fn close(&mut self); }` — `recv_timeout` returns `Ok(None)` on timeout, `Err` on link loss.
  - `pub fn discover() -> std::io::Result<Vec<Candidate>>` — paired devices exposing the v2 UUID in their SDP records (macOS: `IOBluetoothDevice::pairedDevices`, filter by `getServiceRecordForUUID`).
  - `pub fn connect(address: &str) -> std::io::Result<Box<dyn Transport>>` — macOS: find the record, `getRFCOMMChannelID`, `openRFCOMMChannelSync` with a delegate forwarding `rfcommChannelData` into an `mpsc::Sender<Vec<u8>>`; `write` dispatches `writeSync` onto the main queue via `dispatch2::Queue::main().exec_sync`. `connect` must itself be called on the main thread (documented; the app ensures it).
  - `pub struct FakeDevice` implementing `Transport` with `FakeDevice::ult_wear() -> FakeDevice`: answers the handshake exactly as the script in Task 3 Step 1 (plus `04 03` → `05 03 00 08`), ACKs every data frame, echoes `68 17` as `69 17`, `58 03` as `59 03`, `D8 D1` as `D9 D1`, and emits a battery NTFY (`25 00 42 00`) 30 s after connect. Also `FakeDevice::silent_on(&mut self, opcode: u8)` to simulate a refused write.

- [ ] **Step 1: Write the failing fake-device test**

`tests/fake.rs`:

```rust
#[test]
fn fake_device_completes_handshake_through_session() {
    let mut t = FakeDevice::ult_wear();
    let mut s = mdr_session::Session::new(0);
    s.start();
    let mut now = 0;
    let mut connected = false;
    for _ in 0..200 {
        while let Some(out) = s.poll_output() { t.write(&out).unwrap(); }
        if let Some(bytes) = t.recv_timeout(Duration::from_millis(10)).unwrap() {
            now += 5;
            if s.handle_input(&bytes, now).contains(&Event::Connected) { connected = true; break; }
        }
    }
    assert!(connected);
    assert_eq!(s.state().model.as_deref(), Some("ULT WEAR"));
    assert_eq!(s.state().battery.value, Some((67, Charging::NotCharging)));
}

#[test]
fn fake_device_echoes_nc_set_as_ntfy() { /* set Off, pump, assert Confirmed + mode Off */ }

#[test]
fn silent_opcode_leads_to_refused() { /* t.silent_on(0x58); set_ult; pump with now advancing past REPLY_TIMEOUT_MS; assert Refused("no reply") */ }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p mdr-transport`
Expected: compile errors.

- [ ] **Step 3: Implement `lib.rs` (trait, Candidate), `fake.rs`, `discover.rs` + `macos.rs`**

macOS crate deps: `objc2 = "0.6"`, `objc2-foundation = "0.3"`, `objc2-io-bluetooth = "0.3"` (features for `IOBluetoothDevice`, `IOBluetoothRFCOMMChannel`, `IOBluetoothSDPUUID`, `IOBluetoothSDPServiceRecord`), `dispatch2`. Declare the delegate class with `objc2::define_class!` implementing `IOBluetoothRFCOMMChannelDelegate` methods `rfcommChannelData:data:length:` and `rfcommChannelClosed:`. The channel object is retained inside the transport and released in `close`. On Linux/Windows `discover`/`connect` return `Err(Unsupported)` in this phase.

- [ ] **Step 4: Run tests to verify they pass; build the macOS target**

Run: `cargo test -p mdr-transport && cargo clippy -p mdr-transport --all-targets -- -D warnings && cargo build -p mdr-transport --target aarch64-apple-darwin`
Expected: tests pass, clippy clean, macOS build succeeds (the IOBluetooth path is exercised by hand in Task 7).

- [ ] **Step 5: Commit**

```bash
git add crates/mdr-transport
git commit -m "Add mdr-transport with fake device and macOS RFCOMM"
```

---

### Task 5: Tauri app with broker thread, tray, popover window and state events

**Files:**
- Create: `apps/desktop/package.json`, `pnpm-lock.yaml`, `vite.config.ts`, `tsconfig.json`, `svelte.config.js`, `index.html`, `src/main.ts`, `src/App.svelte`, `src/lib/state.ts`, `src/lib/ipc.ts`, `src/styles/tokens.css`, `src/styles/base.css`, `src/assets/fonts/*` (Instrument Sans, Doto, DM Mono `.woff2` + OFL.txt each)
- Create: `apps/desktop/src-tauri/Cargo.toml`, `tauri.conf.json`, `build.rs`, `capabilities/default.json`, `icons/*` (generated), `src/main.rs`, `src/lib.rs`, `src/broker.rs`, `src/commands.rs`, `src/tray.rs`, `src/popover.rs`, `src/audio_macos.rs`
- Test: `apps/desktop/src-tauri/tests/broker.rs`
- Modify: `.gitignore` (already covers `src-tauri/target`, `node_modules`)

**Interfaces:**
- Consumes: `mdr_session::{Session, DeviceState, Event, PairedDevice}`, `mdr_transport::{Transport, FakeDevice, discover, connect, Candidate}`.
- Produces:
  - Rust `broker.rs`: `pub enum BrokerCmd { Connect, Disconnect, SetNc(NcAsmState), SetUlt(UltLevel), SetMultipoint(bool), Refresh }`; `pub fn spawn_broker(app: AppHandle, fake: bool) -> Sender<BrokerCmd>`; the broker loop: `recv_timeout(10 ms)` on the transport, drain commands, `handle_timeout`, write `poll_output`, and after any `StateChanged` emit Tauri event `"state"` with `DeviceState` JSON. On transport error → `mark_disconnected`, then reconnect attempts every 5 s via `discover()` (name contains `ULT WEAR` or `Sony ULT`, else first candidate with the v2 UUID).
  - Tauri commands in `commands.rs`: `get_state() -> DeviceState`, `set_nc(mode: String /* "off"|"nc"|"ambient" */, level: u8, voice_focus: bool)`, `set_ult(level: u8)`, `set_multipoint(on: bool)`, `connect()`, `open_main_window()`.
  - `audio_macos.rs`: `pub fn current_output_device_name() -> Option<String>` (CoreAudio default output device name) used to set `PairedDevice { name: "This Mac", playing: <default output is the headphones> }`; other platforms return None.
  - `tray.rs`: tray icon (template image, black/clear, 18 pt) whose left click toggles the popover; `popover.rs`: window label `popover`, 340×420, `decorations: false`, `transparent: true`, `alwaysOnTop`, `skipTaskbar`, converted to NSPanel with `tauri_nspanel` (non-activating, `floating` level), positioned with `tauri_plugin_positioner::Position::TrayCenter`, hidden on focus loss. Activation policy Accessory at startup.
  - Frontend `ipc.ts`: `listen<DeviceState>("state")`, `invoke("set_nc", …)` etc., typed `DeviceState` mirrored from Rust (`Status` as a tagged union).
  - CLI flag `--fake-device` (env `HAIRLINE_FAKE=1` also accepted) selects `FakeDevice::ult_wear()`.

- [ ] **Step 1: Scaffold the Tauri + Svelte app**

`pnpm create tauri-app@4.7.5 apps/desktop --template svelte-ts --manager pnpm --yes` is acceptable as a starting point; then set `productName: "Hairline"`, `identifier: "com.drake0306.hairline"`, `app.windows: []` (windows are created in code), add `tauri-nspanel = "2.1"` (macOS only) and `tauri-plugin-positioner = { version = "2.4", features = ["tray-icon"] }`, Tauri features `["tray-icon", "image-png"]`. Download the three fonts from `github.com/google/fonts` (`ofl/instrumentsans`, `ofl/doto`, `ofl/dmmono`), convert to woff2 with `pnpm dlx wawoff2` or ship TTF, and copy each OFL.txt beside them.

- [ ] **Step 2: Write the failing broker test**

`src-tauri/tests/broker.rs` tests the broker loop logic with the fake transport and a channel in place of the Tauri event emitter (factor the loop into `pub fn run_broker<E: Fn(&DeviceState)>(transport: Box<dyn Transport>, cmds: Receiver<BrokerCmd>, emit: E, clock: impl FnMut() -> u64)` that returns when it receives `Disconnect`):

```rust
#[test]
fn broker_connects_and_emits_state_then_applies_set_ult() {
    let (tx, rx) = channel();
    let (etx, erx) = channel();
    let t = Box::new(FakeDevice::ult_wear());
    std::thread::spawn(move || run_broker(t, rx, |s| etx.send(s.clone()).unwrap(), monotonic_ms));
    let st = wait_for(&erx, |s| s.connected);
    assert_eq!(st.model.as_deref(), Some("ULT WEAR"));
    tx.send(BrokerCmd::SetUlt(UltLevel::Ult2)).unwrap();
    let st = wait_for(&erx, |s| s.eq_ult.value.as_ref().map(|e| e.ult) == Some(UltLevel::Ult2) && matches!(s.eq_ult.status, Status::Confirmed));
    assert!(st.connected);
    tx.send(BrokerCmd::Disconnect).unwrap();
}

#[test]
fn disconnect_marks_all_unavailable() {
    // FakeDevice::ult_wear() then t.drop_link_after(n_frames) ; assert emitted state has connected == false and battery Unavailable
}
```

- [ ] **Step 3: Run the test to verify it fails**

Run: `cd apps/desktop/src-tauri && cargo test`
Expected: compile errors.

- [ ] **Step 4: Implement `broker.rs`, `commands.rs`, `tray.rs`, `popover.rs`, `audio_macos.rs`, `lib.rs`**

`lib.rs` builds the app: plugins (positioner, nspanel on macOS), `setup` creates the tray and the popover window, sets `ActivationPolicy::Accessory`, parses `--fake-device`, spawns the broker. `commands.rs` forwards to the broker channel and `get_state` returns the last emitted state from a `Mutex<DeviceState>` in Tauri state.

- [ ] **Step 5: Build the popover UI**

`tokens.css` defines the Hairline tokens from Global Constraints as CSS custom properties plus `@font-face` for the three fonts. `App.svelte` renders exactly the "Connected" and "Not connected" popovers from `docs/research/reports/Hairline design screens.html`: device name row, Battery (Doto numeral), Noise boxed switch (NC / AMB / OFF), Level tick bar (20 ticks, click to set), ULT boxed switch (OFF / 1 / 2, orange active), Link list (This Mac ▶ Playing or idle), footer "Open window ⌥⌘N". Boxed switches render `Status`: Pending → striped fill (`repeating-linear-gradient(45deg, var(--ink) 0 4px, #3a3a36 4px 8px)`), Refused → orange outline with the reason as a `title`, Unavailable → opacity 0.35 and disabled. State comes from `ipc.ts`; the initial state from `get_state()`.

- [ ] **Step 6: Run everything**

Run: `cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings && cd apps/desktop && pnpm install && pnpm check && pnpm tauri dev -- --fake-device`
Expected: tests pass; app launches with a tray icon; clicking it shows the popover with "ULT WEAR", 67 %, Ambient active, level 14, ULT 1; clicking ULT 2 shows the striped pending box, then solid orange on 2 within a second; the app has no Dock icon.

- [ ] **Step 7: Commit**

```bash
git add apps/desktop .gitignore
git commit -m "Add Hairline desktop app with broker, tray and popover"
```

---

### Task 6: Push and CI

**Files:**
- Create: `.github/workflows/ci.yml`, `README.md`

- [ ] **Step 1: Write `README.md`** — what it is (one paragraph), status (Phase 1, macOS, WH-ULT900N), how to run (`pnpm tauri dev -- --fake-device`), how to run tests, the "not affiliated with Sony" line, link to the spec. No AI mentions.

- [ ] **Step 2: Write `ci.yml`** — on push and PR: `macos-latest`, Rust stable, pnpm 10, Node 22; steps: `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `pnpm install --frozen-lockfile`, `pnpm check`, `cargo build -p mdr-transport`.

- [ ] **Step 3: Push**

Run: `git add README.md .github && git commit -m "Add README and CI" && git push -u origin main`
Expected: push accepted as Drake0306 (verify with `git log -1 --format=%ae` = `abhinavroy.hello@gmail.com`), CI green on GitHub (`gh run list --limit 1`).

---

### Task 7: Hardware validation on Roy's WH-ULT900N (needs Roy present)

**Files:**
- Create: `docs/hardware-validation.md`
- Modify: `docs/protocol-notes.md` (resolve every **verify** touched)

- [ ] **Step 1: Read-only probe.** With the headphones connected to the Mac and Sound Connect closed on the phone, run `pnpm tauri dev` (real transport) with `HAIRLINE_FRAME_LOG=1` (broker writes every frame, scrubbed, to `captures/local/<date>.log`). Confirm the handshake completes and record model, firmware, both capability lists, battery, NC state, EQ capability (preset names), codec.

- [ ] **Step 2: Controls.** From the popover: NC → Ambient → Off; level 1, 10, 20; voice focus on/off; ULT Off/1/2. Record for each whether the NTFY arrived and what the cup button shows.

- [ ] **Step 3: EQ-type dispute.** Via a temporary debug command, send `58 03` with preset `0x16` (Bass) and bands unchanged, then GET `56 03`; then `58 00` the same; record which one changed the preset. Update `docs/protocol-notes.md`.

- [ ] **Step 4: Table-2 peripheral probe.** Send GET-only `36 02` and `32 02` on table 2; record whether the unit answers or stays silent (deny-list unaffected). Mark proven if it answers.

- [ ] **Step 5: Phone contention.** Open Sound Connect on the phone while connected; record what the Mac sees (link drop vs silence) and that the popover shows "Not connected" and recovers after the phone app closes.

- [ ] **Step 6: Write `docs/hardware-validation.md`** with date, firmware, the results table, and the scrubbed log attached as a fixture under `crates/mdr-session/tests/fixtures/ult900n_fw2_1_0.log` if clean. Commit and push.
