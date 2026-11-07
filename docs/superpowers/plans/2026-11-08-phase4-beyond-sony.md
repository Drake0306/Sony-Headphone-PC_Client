# Phase 4: Beyond Sony — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The parts of the spec's Phase 4 not yet shipped: an Advanced panel (live frame log, capability list, raw command), an opcode probe that walks GET-only commands and records which undeclared ones answer, battery history with a "time left" estimate, and a local socket with a `hairline` command-line tool. Scenes and hotkeys shipped in Phase 2b.

**Architecture:** Everything rides on the existing broker thread and `Session`. The broker gains a bounded frame ring (last 500 frames) it can stream to the window, a recording switch for the existing `FrameLog`, a probe runner that sends one GET at a time through `Session::request` (deny-list and connection gating unchanged, capability gating bypassed only for GETs because that is the point of a probe), a battery sampler that appends to `battery.jsonl`, and a Unix-socket server that maps JSON lines onto the same validated `BrokerCmd`s the Tauri commands use. `mdr-proto` gains the function-name table and the GET catalogue so the app and CLI never hard-code opcodes. The window gains an Advanced screen, shown only when Settings enables it.

**Tech Stack:** as before; `serde_json` lines over `std::os::unix::net::UnixListener` (macOS first; Windows returns Unsupported); `clap` 4 for `mdr-cli`.

**Spec:** `docs/superpowers/specs/2026-11-07-ult-wear-desktop-app-design.md` — Phase 4 line, "Room to grow", deny-list granularity (power off and reset behind confirmation; **owner ruling 2026-11-08: no reset anywhere in the app**), "Capability gating … declared by `06/07`, or proven by a read-only probe on this unit (a per-unit allowlist stored with the device record). SETs on proven-but-undeclared commands stay behind the Advanced panel", "Time left needs battery history … Blank, never invented."

## Global Constraints

- No AI attribution; plain commits; every commit passes `cargo test --workspace`, clippy `-D warnings`, `cargo fmt --all -- --check`, `cargo check -p mdr-transport --target x86_64-pc-windows-msvc`, `pnpm check`, `pnpm test`.
- Deny-list (`mdr-proto::deny`) is absolute in every new path: raw command, probe, socket. No new bypass; `request_confirmed` stays limited to power off and is not exposed through the socket or raw command. Nothing offers a reset.
- Probe sends GETs only: opcodes from the catalogue, never anything whose libmdr role is SET; one request in flight; 1.5 s reply wait; 150 ms gap; stops on disconnect; runs only after the owner clicks Run and confirms, with the headphones connected.
- Nothing static on screen: every row comes from device state, files or the catalogue. Blank when unknown.
- Colours via tokens only; Hairline design; the Advanced screen follows the Settings screen's two-column layout (no mockup exists; ruling).
- Per-unit "proven" list lives in `devices/<normalised address>.json` beside `settings.json`; loaded into the session with `mark_proven` right after connect; written when the probe adds entries.
- Time left: linear fit over the current discharge segment (samples since the last charging or gap > 30 min); shown only with ≥ 30 min span and ≥ 2 % drop; rounded to 10 min; otherwise blank.

## Review Focus

1. A probe must never send a SET or a denied byte even if the catalogue is edited wrongly — Task 3 `probe_catalogue_is_get_only` checks every catalogue opcode against the libmdr GET list and `denied()`.
2. A raw SET from the Advanced panel without the typed confirmation must be refused before reaching the broker — Task 1 `raw_set_needs_confirmation`.
3. A proven function recorded on one unit must not apply to another address — Task 1 `proven_list_is_per_address`.
4. Time left must stay blank while charging, after a gap, or on too little data — Task 4 `time_left_blank_cases`.
5. A socket client sending malformed JSON or an unknown control must get an error line and the connection must survive — Task 5 `socket_rejects_bad_input_and_stays_up`.

---

### Task 1: Advanced backend — names, proven store, frame ring, recording, raw command, setting

**Files:**
- Create: `crates/mdr-proto/src/function.rs` (name table), `apps/desktop/src-tauri/src/device_record.rs`, `apps/desktop/src-tauri/src/frame_ring.rs`
- Modify: `crates/mdr-proto/src/lib.rs`, `apps/desktop/src-tauri/src/{broker,commands,settings,lib}.rs`
- Test: `crates/mdr-proto/src/function.rs` (unit), `apps/desktop/src-tauri/tests/{device_record,frame_ring,advanced}.rs`

**Interfaces:**
- Produces: `mdr_proto::function::name(table: Table, id: u8) -> Option<&'static str>` (T1 and T2 support-function names from `docs/protocol-notes.md` "Support function bytes" plus the full libmdr FunctionType lists the implementer reads from `docs/research/notes`); `device_record::{load(dir, address) -> DeviceRecord { proven: Vec<(Table,u8)> }, save(dir, address, &DeviceRecord)}`; `FrameRing::new(500)`, `push(FrameEntry { t_ms, dir: Sent|Received, table, bytes })`, `snapshot() -> Vec<FrameEntry>`; `BrokerCmd::{WatchFrames(bool), RecordFrames(bool), Raw{table, bytes, confirmed_set: bool}}`; events `"frames"` (`Vec<FrameEntry>`, emitted on each new frame while watched, as a snapshot capped at 500) and `"recording"` (`{ path: Option<String> }`); Tauri commands `capability_list() -> Vec<CapabilityRow { table, id, name: Option<String>, declared: bool, proven: bool }>` (computed from `DeviceState.capabilities_t1/t2/proven`), `watch_frames(on)`, `record_frames(on) -> Option<path>`, `send_raw(table, hex, confirm: String) -> Result<(), String>` (a payload whose opcode is a SET per the libmdr role table requires `confirm == "SEND"`, else `Err("type SEND to send a SET")`; deny-list errors pass through from the session); `AppSettings.show_advanced: bool` (default false, serde default).
- Consumes: `FrameLog` (`frame_log.rs`), `Session::mark_proven`, `parse_raw`.

- [ ] Failing tests: `function_names_cover_the_declared_set` (every id in the notes' T1/T2 lists has a name), `proven_list_is_per_address`, `device_record_round_trip`, `frame_ring_keeps_the_last_500`, `raw_set_needs_confirmation`, `raw_get_needs_no_confirmation`, `watch_frames_streams_sent_and_received` (fake device; at least the handshake frames appear), `record_frames_creates_and_closes_a_file`.
- [ ] Implement; the broker loads the device record after connect and calls `mark_proven`; `debug_raw`/`HAIRLINE_DEBUG` are removed in favour of `send_raw` (gated by `show_advanced` in the frontend, deny-list in the session).
- [ ] Green + full checks. Commit `Add the Advanced backend: names, proven store, frame ring, raw command`.

---

### Task 2: Advanced screen

**Files:**
- Create: `apps/desktop/src/views/Advanced.svelte`, `apps/desktop/src/lib/advanced.ts` (+ tests), `src/lib/controls/FrameList.svelte`
- Modify: `src/views/{Main,Settings}.svelte`, `src/lib/{ipc,settings}.ts`

- [ ] Settings → "This app" block: toggle "Advanced panel" (`show_advanced`). When on, Main's nav gains "Advanced" (last). Screen, two columns like Settings: left "Capabilities" (two `LinkList`-style tables T1/T2: id hex, name or `--`, chips DECLARED / PROVEN, faded when neither), "Raw command" (table boxes T1·T2, hex field, a confirm field that appears only when the parsed opcode is a SET, Send button, result note: ACK/RET hex or the error text); right "Frame log" (live tail of the last 500 frames, newest at the bottom, mono, `→`/`←` direction, hex grouped, auto-scroll; buttons Clear (local) and Record / Stop with the file path shown while recording), "Probe" block placeholder wired in Task 3 (hidden until then, not a static card). `hexGroups(bytes)`, `isSetOpcode(table, bytes)`, `frameLine(entry)` helpers tested. Watching starts when the screen mounts and stops when it unmounts.
- [ ] `pnpm check`/`pnpm test`; fake walkthrough. Commit `Add the Advanced screen`.

---

### Task 3: Opcode probe

**Files:**
- Create: `crates/mdr-proto/src/probe.rs` (`GET_CATALOGUE: &[ProbeSpec { table, opcode, inquired: &[u8], name }]` built from the notes' GET columns: T1 `00,02,04,06,10,12,20,22,26,50,56,5A,60,66,A0,A2,A6,D0,D2,D6,E0,E2,E6,F0,F2,F6,FA`, T2 `06,32,36,46,50..` with the inquired ids each type knows from libmdr; `is_get(table, opcode) -> bool` from the libmdr role tables), `apps/desktop/src-tauri/src/probe.rs`
- Modify: `broker.rs`, `commands.rs`, `Advanced.svelte`, `advanced.ts`
- Test: `crates/mdr-proto/src/probe.rs` unit (`probe_catalogue_is_get_only`, `probe_catalogue_has_no_denied_bytes`), `apps/desktop/src-tauri/tests/probe.rs` (fake device scripted to answer two undeclared GETs: results mark exactly those proven, event sequence `running → done`, nothing sent after Stop, SET never appears in the fake's received frames)

- [ ] `BrokerCmd::{ProbeStart, ProbeStop}`; runner state machine in the broker loop (one request, wait for RET `opcode+1` with matching inquired byte or 1.5 s, record `ProbeResult { table, opcode, inquired, reply: Option<Vec<u8>>, declared: bool }`, 150 ms gap); on finish: `mark_proven` for undeclared answered functions (function id = the support-function the opcode belongs to, mapped via `mdr_proto::function::for_opcode`), save the device record, write `captures/local/probe-<ts>.json`, emit `"probe"` `{ state: Idle|Running{done,total}|Done, results }`. Commands `probe_start() / probe_stop()`. UI: Probe block with Run (confirm box "The probe sends read-only GET requests one at a time. Type RUN.") and Stop, progress `n / total`, results table (opcode, name, DECLARED / PROVEN / silent, reply hex), link to the JSON file.
- [ ] Commit `Add the opcode probe`.

---

### Task 4: Battery history and time left

**Files:**
- Create: `apps/desktop/src-tauri/src/battery_history.rs`
- Modify: `broker.rs`, `commands.rs`, `crates/mdr-session/src/state.rs`? **No** — keep `DeviceState` unchanged; expose `BatteryEstimate { minutes_left: Option<u32>, samples: usize, since: Option<u64> }` via event `"battery"`; `src/views/Overview.svelte` (time left line), `Settings.svelte` ("Battery history: N samples since <date> · Clear")
- Test: `tests/battery_history.rs` (`time_left_blank_cases`, `time_left_from_a_steady_drain` (100→90 over 60 min → 540 min ±10), `samples_append_only_on_change`, `clear_removes_the_file`)

- [ ] `append(dir, Sample { t: u64, level: u8, charging: bool })` on change; `estimate(samples, now) -> BatteryEstimate`; broker emits after each append and on connect; `clear_battery_history()` command.
- [ ] Commit `Record battery history and estimate time left`.

---

### Task 5: Local socket and `hairline` CLI

**Files:**
- Create: `apps/desktop/src-tauri/src/socket.rs`, `crates/mdr-cli/` (bin name `hairline`), `crates/mdr-cli/src/{main,proto}.rs`
- Modify: `lib.rs` (spawn server), `commands.rs` (share validation via a `pub fn apply_control(name, value) -> Result<BrokerCmd, String>`), `Cargo.toml` workspace
- Test: `tests/socket.rs` (`socket_rejects_bad_input_and_stays_up`, `socket_state_round_trip`, `socket_set_uses_the_same_validation`), `crates/mdr-cli` unit tests for argument → request mapping

- [ ] Protocol: one JSON object per line. Requests `{"cmd":"state"}` → `{"ok":true,"state":<DeviceState>}`; `{"cmd":"set","control":"nc|ambient_level|voice_focus|ult|volume|eq_preset|dsee_auto_off","value":…}` → `{"ok":true}` or `{"ok":false,"error":…}`; `{"cmd":"scene","name":…}`; `{"cmd":"scenes"}`; `{"cmd":"battery"}`. Path `<app data dir>/hairline.sock`, mode 0600, removed on exit; `cfg(not(unix))` → server not started. CLI: `hairline state|nc <nc|ambient|off> [--level N] [--voice on|off]|ult <0|1|2>|volume <0..30>|eq <name|hex>|scene <name>|scenes|battery` with `--json`; exit 1 with the error text when the app is not running.
- [ ] Commit `Add the local socket and the hairline CLI`.

---

### Task 6: Docs and validation

- [ ] README: "Advanced", "Probe", "Battery history", "Command line" sections; Status: Phase 4 done, "Next: Windows tray app, then Linux". `docs/hardware-validation.md` §8 Phase 4 checklist (owner present: probe run and results, a raw GET, a raw SET with confirmation on a declared function only, time left after an hour, CLI `hairline state`). `docs/protocol-notes.md`: probe catalogue pointer. Commit `Document Phase 4`.
