# Phase 2: Main window — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The full Hairline window (Overview, Sound, Noise, Devices, Settings, with a Scenes placeholder) driven by real headphone state, with Paper/Ink/System themes, launch at login, and the Dock icon shown only while the window is open, validated on the WH-ULT900N.

**Architecture:** Extend the protocol library bottom-up (new payload codecs in `mdr-proto`, new tracked fields and setters in `mdr-session`, new fake-device replies in `mdr-transport`), then the app backend (broker commands, Tauri commands, a small JSON settings store, autostart), then the Svelte frontend as a router plus shared control components plus one component per screen, matching `docs/research/reports/Hairline design screens.html` exactly. Every control renders the four states (confirmed, pending, refused, unavailable).

**Tech Stack:** as Phase 1 (Rust stable, Tauri 2.12, Svelte 5, Vite 8, pnpm), plus `tauri-plugin-autostart 2.x`. libmdr enum values referenced below were read from the generated headers; where a value is marked **verify**, the hardware step confirms it.

**Spec:** `docs/superpowers/specs/2026-11-07-ult-wear-desktop-app-design.md` (Phase 2 paragraph, UI section, builder-check resolutions). Hardware facts: `docs/hardware-validation.md`, `docs/protocol-notes.md`.

## Global Constraints

- No AI attribution anywhere (see `CLAUDE.md`); commits as the repo identity, no trailers.
- `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all -- --check`, and `cd apps/desktop && pnpm check` pass at every commit. Rust edits restart a running `pnpm tauri dev`; frontend edits hot-reload.
- Deny-list and capability gating unchanged; every new write goes through `Session::submit`'s gating. Reset (`F8 09`) stays denied; the Settings "Reset" row renders unavailable with the note "Phase 4".
- Read-modify-write for anything in the EQ family: `set_eq` and `set_ult` echo the last RET's other fields.
- Tokens (from `tokens.css`): paper `#efede6`, ink `#161614`, forest `#4a5550`, hairline `#cfccc2`, muted `#6f726d`, ULT orange `#ff6600`, box `#f8f7f3`. **Ink theme**: paper and ink swap (`--paper: #161614`, `--ink: #efede6`), box `#1f1f1d`, hairline `#3a3a36`, muted `#9a9a92`, forest and orange unchanged. "System" follows `prefers-color-scheme`.
- Window: label `main`, 880×560 minimum, resizable, Hairline nav tabs Overview · Sound · Noise · Devices · Scenes · Settings; Scenes shows a placeholder until the modes module lands.
- Value enums (libmdr): `OnOffSettingValue` ON=0x00 OFF=0x01 (wear pause `F7 01`, touch panel `D7 D2`, sidetone `D7 D3`); `UpscalingTypeAutoOff` OFF=0x00 AUTO=0x01 (`E7 01`); `VoiceAssistant` 0x30 phone, 0x31 Google, 0x32 Alexa, 0x34 Sony, 0xFF none (`F7 04`); auto power-off elements 0x00 5 min, 0x04 15 min, 0x01 30 min, 0x02 60 min, 0x03 180 min, 0x10 when removed from ears, 0x11 disabled (`27 05`); `PlaybackStatus` 0x01 play, 0x02 pause, 0x03 stop; voice guide volume `47 20 <i8 -2..2>`; preset ids and names: 0x00 Off, 0x10 Bright, 0x11 Excited, 0x12 Mellow, 0x13 Relaxed, 0x14 Vocal, 0x15 Treble Boost, 0x16 Bass Boost, 0x17 Speech, 0xA0 Manual, 0xA1 Custom 1, 0xA2 Custom 2 (the device sends empty names).
- Hardware-observed replies the fake device must reproduce: `12 03`→`13 03 01 01`; `E6 01`→`E7 01 01`; `E6 02`→`E7 02 00`; `F6 01`→`F7 01 01`; `F6 04`→`F7 04 FF`; `F6 0D`→`F7 0D 02 00 00`; T2 `46 20`→`47 20 02`; `A2 01`→`A3 01 00 02 00`; `D6 D2`→`D7 D2 00 01`; `D6 D3`→`D7 D3 00 00`; `D0 D2`→boolean `TOUCH_PANEL_SETTING`; `D0 D3`→boolean `SIDETONE_SETTING`/`SIDETONE_SETTING_SUMMARY`; `26 05`→`27 05 10 11`; every SET echoes as the NTFY opcode (SET+1) with the same payload.

## Review Focus

1. Dragging an EQ fader must not send a frame per pixel: one SET on pointer release (or 150 ms after the last keyboard step), with the handle following the pointer — pinned in Task 6 (`faders_commit_once_on_release`).
2. Changing the EQ preset must keep the ULT level and the current bands the device reported, never zeros — pinned in Task 2 (`set_eq_preset_keeps_ult_and_bands`).
3. Switching theme to Ink and back must leave no literal light-theme colour on screen (every colour is a token) — pinned in Task 5 by the `tokens.css` structure and a `pnpm check`-time grep step.
4. Closing the main window must return the app to Accessory (no Dock icon) and the popover must keep working — pinned in Task 4 (`dock_policy_follows_main_window`, a logic test of the policy function).
5. A refused write (device answers nothing) on any Settings row must show the orange outline with "no reply" and revert to the device's value — pinned in Task 2's session behaviour (already tested) and Task 6's shared control rendering (`boxes_render_refused_state`).

---

### Task 1: `mdr-proto` — Phase 2 messages, builders and preset names

**Files:**
- Modify: `crates/mdr-proto/src/opcode.rs`, `src/types.rs`, `src/message.rs`, `src/request.rs`, `src/lib.rs`
- Test: `crates/mdr-proto/tests/decode.rs`, `tests/requests.rs`

**Interfaces:**
- Produces:
  - `pub fn preset_name(id: u8) -> &'static str` (table in Global Constraints; unknown ids → "Preset 0xNN" is not allowed: return `"Unknown"`).
  - New `Message` variants: `Dsee { on: bool }` (from `13 03 <type> <status>` and `15 03 …`; on = status != 0), `DseeAutoOff { auto: bool }` (`E7 01`/`E9 01`), `ConnectionMode { value: u8 }` (`E7 02`/`E9 02`, first byte after type), `WearPause { on: bool }` (`F7 01`/`F9 01`, on = byte == 0x00), `VoiceAssistant { value: u8 }` (`F7 04`/`F9 04`), `QuickAccess { functions: Vec<u8> }` (`F7 0D`/`F9 0D`, count-prefixed), `VoiceGuideVolume { value: i8 }` (T2 `47 20`/`49 20`), `Playback { status: u8 }` (`A3 01 …`: the status byte is the **second** byte after the type, i.e. payload[3]; **verify**).
  - New `Group` variants: `Dsee, DseeAutoOff, ConnectionMode, WearPause, VoiceAssistant, QuickAccess, VoiceGuide, Playback`.
  - New builders (capability in parentheses): `dsee_status_get()` (0x13, `[0x12,0x03]`), `dsee_auto_off_get()`/`dsee_auto_off_set(auto: bool)` (0xE2, `[0xE6,0x01]`/`[0xE8,0x01,auto as u8]`), `connection_mode_get()`/`connection_mode_set(value: u8)` (0xE3, `[0xE6,0x02]`/`[0xE8,0x02,value]`), `wear_pause_get()`/`wear_pause_set(on: bool)` (0xF1, `[0xF6,0x01]`/`[0xF8,0x01, if on {0} else {1}]`), `voice_assistant_get()`/`voice_assistant_set(value: u8)` (0xF4, `[0xF6,0x04]`/`[0xF8,0x04,value]`), `quick_access_get()`/`quick_access_set(functions: &[u8])` (0xFD, `[0xF6,0x0D]`/`[0xF8,0x0D,n,fn…]`), `voice_guide_volume_get()`/`voice_guide_volume_set(v: i8)` (T2, 0x42, `[0x46,0x20]`/`[0x48,0x20,v as u8]`), `playback_status_get()` (0xA1, `[0xA2,0x01]`), `multipoint_commit()` (`[0x98,0x00,0x06,0x01]`, capability `Some(0xD1)`, group `GeneralSetting(0xD1)`).

- [ ] **Step 1: Write the failing tests** — in `tests/decode.rs`: `dsee_status_on` (`[0x13,0x03,0x01,0x01]` → `(Ret, Dsee{on:true})`), `dsee_auto_off_value` (`[0xE7,0x01,0x01]` → `DseeAutoOff{auto:true}`), `connection_mode_value` (`[0xE7,0x02,0x00]` → `ConnectionMode{value:0}`), `wear_pause_off_is_one` (`[0xF7,0x01,0x01]` → `WearPause{on:false}`), `voice_assistant_none` (`[0xF7,0x04,0xFF]` → `VoiceAssistant{value:0xFF}`), `quick_access_two_slots` (`[0xF7,0x0D,0x02,0x00,0x00]` → `QuickAccess{functions:vec![0,0]}`), `voice_guide_volume_t2` (`decode(T2,[0x47,0x20,0x02])` → `VoiceGuideVolume{value:2}`; `[0x47,0x20,0xFE]` → `-2`), `playback_paused` (`[0xA3,0x01,0x00,0x02,0x00]` → `Playback{status:2}`), `preset_names` (`preset_name(0x16)=="Bass Boost"`, `preset_name(0xA2)=="Custom 2"`, `preset_name(0x05)=="Unknown"`). In `tests/requests.rs`: `phase2_builders_bytes` asserting each builder's bytes, table, capability and group as listed above, including `voice_guide_volume_set(-2).bytes == [0x48,0x20,0xFE]` and `multipoint_commit()` passing `denied()`.
- [ ] **Step 2: Run** `cargo test -p mdr-proto` → compile errors.
- [ ] **Step 3: Implement** the variants, builders and `preset_name`; extend `decode`'s match (Kind by low nibble as before).
- [ ] **Step 4: Run** `cargo test -p mdr-proto && cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --all` → green.
- [ ] **Step 5: Commit** `Add Phase 2 message codecs, builders and preset names`.

---

### Task 2: `mdr-session` — new tracked fields, handshake reads and setters

**Files:**
- Modify: `crates/mdr-session/src/state.rs`, `src/handshake.rs`, `src/session.rs`, `src/lib.rs`
- Test: `crates/mdr-session/tests/conversation.rs`, `tests/gating.rs`, `tests/common/mod.rs`

**Interfaces:**
- Consumes: Task 1.
- Produces:
  - `DeviceState` gains: `dsee: Tracked<bool>`, `dsee_auto_off: Tracked<bool>`, `connection_mode: Tracked<u8>`, `wear_pause: Tracked<bool>`, `voice_assistant: Tracked<u8>`, `quick_access: Tracked<Vec<u8>>`, `voice_guide_volume: Tracked<i8>`, `playback: Option<u8>` (not tracked), `touch_panel: Tracked<bool>` and `sidetone: Tracked<bool>` (derived from `GsBool{0xD2}` / `GsBool{0xD3}` the same way `multipoint` derives from 0xD1; keep `gs` as is).
  - Handshake additions, each only when its capability is declared, after the existing reads: `dsee_status_get` (0x13), `dsee_auto_off_get` (0xE2), `connection_mode_get` (0xE3), `wear_pause_get` (0xF1), `voice_assistant_get` (0xF4), `quick_access_get` (0xFD), `voice_guide_volume_get` (T2 0x42), `playback_status_get` (0xA1).
  - Setters (all `-> Result<(), SessionError>`): `set_eq_preset(preset: u8)`, `set_eq_bands(bands: Vec<u8>)` (both read-modify-write from `eq_ult.value`, keeping `ult`; `set_eq_bands` requires `bands.len() == eq_capability.bands` else `Err(SessionError::InvalidValue)`), `set_volume(v: u8)` (0..=30 else `InvalidValue`), `set_dsee_auto_off(auto: bool)`, `set_connection_mode(v: u8)`, `set_auto_power_off(v: u8)` (one of the seven element values else `InvalidValue`), `set_wear_pause(on)`, `set_voice_assistant(v)`, `set_quick_access(functions: Vec<u8>)`, `set_voice_guide_volume(v: i8)` (-2..=2), `set_touch_panel(on)`, `set_sidetone(on)` (via `gs_set_bool(0xD2|0xD3, on)`), `set_multipoint(on)` (queues `gs_set_bool(0xD1,on)` then `multipoint_commit()`).
  - `SessionError::InvalidValue` added.
  - `Message::Playback` updates `state.playback` and emits `StateChanged` without touching any `Tracked`.

- [ ] **Step 1: Extend `tests/common/mod.rs`'s script** with the hardware-observed replies from Global Constraints (so `connected_session()` declares caps `20 23 53 6B 12 13 D1 D2 D3 25 A1 E2 E3 F1 F4 FD` on T1 and `42 50 F2` on T2) and write the failing tests in `tests/conversation.rs`: `handshake_reads_phase2_fields` (after connect: `dsee.value==Some(true)`, `dsee_auto_off.value==Some(true)`, `connection_mode.value==Some(0)`, `wear_pause.value==Some(false)`, `voice_assistant.value==Some(0xFF)`, `quick_access.value==Some(vec![0,0])`, `voice_guide_volume.value==Some(2)`, `playback==Some(2)`, `touch_panel.value==Some(false)`, `sidetone.value==Some(true)`, `auto_power_off.value==Some(0x10)`, all tracked statuses Confirmed); `set_eq_preset_keeps_ult_and_bands` (state eq `A2/Ult1/[10,13,10,14,20,12]`; `set_eq_preset(0x16)` sends `58 03 16 01 06 0A 0D 0A 0E 14 0C`); `set_eq_bands_validates_length` (5 bands → `Err(InvalidValue)`, 6 bands sends `58 03 A2 01 06 <bands>`); `set_multipoint_sends_bool_then_commit` (two frames, `D8 D1 00 00` then `98 00 06 01`, second only after the first is ACKed); `set_voice_guide_volume_t2` (frame type Command2, bytes `48 20 FE` for −2); `set_wear_pause_sends_inverted_enum` (`F8 01 00` for on); `invalid_volume_rejected` (`set_volume(31)` → `InvalidValue`). In `tests/gating.rs`: `phase2_setters_are_gated` (with caps `[0x20]` only, `set_dsee_auto_off(true)` → `Unavailable(0xE2)` and `dsee_auto_off.status == Unavailable`).
- [ ] **Step 2: Run** `cargo test -p mdr-session` → compile errors.
- [ ] **Step 3: Implement** state fields, `status_mut`/`apply` arms for the new messages, handshake steps, setters, `InvalidValue`.
- [ ] **Step 4: Run** `cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --all` → green (the transport fake tests and the broker tests must still pass; if the longer handshake changes the broker test's `drop_link_after` margin, adjust it in Task 3, not here, and say so in the report).
- [ ] **Step 5: Commit** `Track Phase 2 settings in the session and add their setters`.

---

### Task 3: `mdr-transport` fake device and app backend (broker, commands, settings store, autostart, Dock policy)

**Files:**
- Modify: `crates/mdr-transport/src/fake.rs` (new replies from Global Constraints; SET echoes for `E8`, `F8`, `A8`, `28`, `D8`, T2 `48`), `apps/desktop/src-tauri/Cargo.toml` (`tauri-plugin-autostart = "2"`), `src-tauri/src/lib.rs`, `src/broker.rs`, `src/commands.rs`, `src/popover.rs`
- Create: `apps/desktop/src-tauri/src/settings.rs`, `src/dock.rs`
- Test: `apps/desktop/src-tauri/tests/broker.rs` (extend), `apps/desktop/src-tauri/tests/settings.rs`, `apps/desktop/src-tauri/tests/dock.rs`

**Interfaces:**
- Produces:
  - `BrokerCmd` gains `SetEqPreset(u8)`, `SetEqBands(Vec<u8>)`, `SetVolume(u8)`, `SetDseeAutoOff(bool)`, `SetConnectionMode(u8)`, `SetAutoPowerOff(u8)`, `SetWearPause(bool)`, `SetVoiceAssistant(u8)`, `SetQuickAccess(Vec<u8>)`, `SetVoiceGuideVolume(i8)`, `SetTouchPanel(bool)`, `SetSidetone(bool)`; `SetMultipoint` now calls `session.set_multipoint`.
  - Tauri commands, one per setter, named `set_eq_preset(preset: u8)`, `set_eq_bands(bands: Vec<u8>)`, `set_volume(volume: u8)`, `set_dsee_auto_off(auto: bool)`, `set_connection_mode(value: u8)`, `set_auto_power_off(value: u8)`, `set_wear_pause(on: bool)`, `set_voice_assistant(value: u8)`, `set_quick_access(functions: Vec<u8>)`, `set_voice_guide_volume(value: i8)`, `set_touch_panel(on: bool)`, `set_sidetone(on: bool)`; input validated at the edge like the existing ones (`Err(String)` on bad values).
  - `settings.rs`: `#[derive(Serialize, Deserialize, Clone, PartialEq)] pub struct AppSettings { pub theme: Theme /* Paper | Ink | System */, pub launch_at_login: bool, pub hide_dock_icon: bool }` with defaults `System, false, true`; `pub fn load(dir: &Path) -> AppSettings` (missing/corrupt file → defaults), `pub fn save(dir: &Path, s: &AppSettings) -> io::Result<()>` (file `settings.json`, pretty JSON); Tauri commands `get_settings() -> AppSettings`, `set_settings(s: AppSettings) -> Result<(), String>` which also applies: `launch_at_login` via `tauri_plugin_autostart::ManagerExt` (`enable()`/`disable()`), and emits `"settings"` so every window re-reads the theme. Settings dir = `app.path().app_config_dir()`.
  - `dock.rs`: `pub fn policy(main_window_open: bool, hide_dock_icon: bool) -> ActivationPolicyKind` (`Regular` when the main window is open or `hide_dock_icon` is false, else `Accessory`), applied in `popover.rs`/`lib.rs` on main-window `Created`/`Destroyed`/`CloseRequested` and on settings change (macOS only; `set_activation_policy` must run on the main thread via `app.run_on_main_thread`).
  - `open_main_window` creates or focuses the 880×560 window (`min_inner_size(880,560)`, title "Hairline", `index.html?view=main`) and brings the app forward (`app.show()`/`set_focus`).
  - The frame-log and `debug_raw` behaviour unchanged.

- [ ] **Step 1: Failing tests** — `tests/broker.rs`: `broker_applies_eq_preset_and_dsee` (fake device; send `SetEqPreset(0x16)` → state `eq_ult.value.preset == 0x16` Confirmed with ULT still `Ult1`; send `SetDseeAutoOff(false)` → `dsee_auto_off.value == Some(false)` Confirmed); `tests/settings.rs`: `load_defaults_when_missing`, `round_trip_save_load`, `corrupt_file_falls_back_to_defaults`; `tests/dock.rs`: `dock_policy_follows_main_window` (`policy(false,true)==Accessory`, `policy(true,true)==Regular`, `policy(false,false)==Regular`).
- [ ] **Step 2: Run** `cargo test -p hairline` (the app crate name is whatever `src-tauri/Cargo.toml` says; use it) → compile errors.
- [ ] **Step 3: Implement** fake replies, broker commands, Tauri commands, settings store, autostart plugin registration (`tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, None)`), dock policy wiring, main window creation, capability entry `autostart:default`.
- [ ] **Step 4: Run** the full workspace checks → green; `pnpm tauri dev -- -- --fake-device` opens the main window from the popover footer, the Dock icon appears while it is open and disappears when it closes (describe what you observed).
- [ ] **Step 5: Commit** `Add Phase 2 broker commands, settings store, autostart and Dock policy`.

---

### Task 4: Frontend shell — router, theme, settings, shared controls

**Files:**
- Create: `apps/desktop/src/lib/settings.ts`, `src/lib/theme.ts`, `src/lib/controls/Boxes.svelte`, `Ticks.svelte`, `Toggle.svelte`, `Row.svelte`, `LinkList.svelte`, `Headphone.svelte`, `src/views/Popover.svelte`, `src/views/Main.svelte`
- Modify: `src/App.svelte` (becomes the router: `?view=main` → `Main`, else `Popover`; moves the current popover markup into `Popover.svelte` using the shared controls), `src/lib/state.ts` (mirror the new `DeviceState` fields and `AppSettings`), `src/lib/ipc.ts` (all new commands, `getSettings`/`setSettings`, `onSettings`), `src/styles/tokens.css` (Ink + System), `src/styles/base.css`
- Test: `apps/desktop/src/lib/controls/*.test.ts` with Vitest (add `vitest` + `@testing-library/svelte` dev deps and a `pnpm test` script)

**Interfaces:**
- Produces:
  - `Boxes.svelte` props: `options: {label: string, value: T, orange?: boolean}[]`, `value: T | null`, `status: Status`, `disabled?: boolean`, `onpick: (v: T) => void`, `width?: string`; renders the boxed switch with `.on`, `.pending` (striped), `.refused` (orange outline + `title` reason), `.unavailable` (faded, no handler).
  - `Ticks.svelte` props: `count: number`, `value: number | null`, `status`, `disabled`, `onpick`; `size: "sm" | "lg"`.
  - `Toggle.svelte` props: `on: boolean | null`, `status`, `disabled`, `onpick`.
  - `Row.svelte`: three-column row (`label`, content slot, `aside` slot) used by Overview and Settings lists; `LinkList.svelte`: the mono two-column list; `Headphone.svelte`: the SVG silhouette with `color: 1|2|8` → fill tint (Black `#161614` on paper `#2a2a28` panel; Off White panel `#d9d6cc` with silhouette `#efede6`; Forest Gray panel `#4a5550` with silhouette `#efede6`), used as the Overview photo block.
  - `theme.ts`: `applyTheme(t: Theme)` sets `data-theme` on `<html>` (`paper` | `ink` | removes it for system); `tokens.css` defines the Ink values under `:root[data-theme="ink"]` and under `@media (prefers-color-scheme: dark) :root:not([data-theme="paper"])`.
  - `settings.ts`: a Svelte store fed by `getSettings()` and the `"settings"` event.

- [ ] **Step 1: Failing component tests** — `Boxes.test.ts`: `boxes_render_refused_state` (status `{Refused:{reason:"no reply"}}` → the active box has class `refused` and `title="no reply"`), `boxes_pending_is_striped_and_inert` (status Pending → class `pending`, click does not call `onpick`), `boxes_unavailable_is_disabled`; `Ticks.test.ts`: `ticks_click_reports_one_based_level`.
- [ ] **Step 2: Run** `pnpm test` → fails.
- [ ] **Step 3: Implement** the controls, router, theme, settings store, state/ipc mirrors; move the popover to `Popover.svelte` with no behaviour change; add the Ink tokens.
- [ ] **Step 4: Run** `pnpm test && pnpm check` → green; grep step: `grep -nE '#[0-9a-fA-F]{3,6}' apps/desktop/src/**/*.svelte apps/desktop/src/styles/base.css` must return nothing (all colours via tokens; `tokens.css` and `Headphone.svelte`'s three panel tints are the only allowed literals).
- [ ] **Step 5: Commit** `Add the window shell, themes and shared Hairline controls`.

---

### Task 5: Screens — Overview, Noise, Devices, Settings, Scenes placeholder

**Files:**
- Create: `src/views/Overview.svelte`, `Noise.svelte`, `Devices.svelte`, `Settings.svelte`, `Scenes.svelte`
- Modify: `src/views/Main.svelte` (tabs + status label "Connected · FW x.y.z" / "Not connected")

**Interfaces:** consume Task 4 controls and `ipc.ts`.

- [ ] **Step 1: Build each screen to the mockup**, with these hardware-driven adjustments:
  - **Overview**: photo block (`Headphone.svelte` tinted by `state.color`, caption `WH-ULT900N · <colourway>` and `ULT WEAR`), rows Battery (Doto numeral; "charging" when `Charging`), Noise (Boxes), Level (Ticks, gated to Ambient as in the popover), ULT (Boxes), Equalizer (mini faders read-only + preset name via `preset_name`), Codec (`LDAC`/`AAC`/`SBC` in Doto + "DSEE on/off"), then the Link list: This Mac ▶ Playing/idle, Multipoint on/off, Auto off (`5 min`/`15 min`/`30 min`/`60 min`/`180 min`/`When removed`/`Off` from the element value), Wear pause on/off.
  - **Noise**: big Boxes (Noise cancelling / Ambient / Off), large Ticks with the Doto numeral and the "1 · quiet room … 20 · street" labels, Focus on voice toggle (gated to Ambient). Drop "Quick Attention" (a touch gesture, not a setting). The "Cup button cycles through" block renders **unavailable** with note "not readable on this model yet"; the "Automatic" block shows the Scenes placeholder text.
  - **Devices**: Connected now: "This Mac" card with codec and ▶ Playing/idle; Multipoint toggle (`set_multipoint`), "Switch to whichever starts playing" row unavailable ("modes module"); Paired list and "Enter pairing mode" render unavailable with the note "the ULT WEAR does not expose its device list"; "This headphone" link list: Model `WH-ULT900N`, Firmware, Bluetooth `5.2 · channel 18` (address from state if present else blank), Colourway.
  - **Settings**: Headphones column: Auto power off (Boxes: 5 · 15 · 30 · 60 · 180 · Removed · Never → element values), Pause when taken off (Toggle), Touch panel (Toggle), Sidetone (Toggle, new row), Voice guide (Boxes −2 · −1 · 0 · +1 · +2 labelled Quiet … Loud), Quick Access (Boxes from the current `quick_access` values: show "Off" selected when both slots are 0; other assignments render as their raw id until the capability is decoded in a later phase), Voice assistant (Boxes Phone · Google · Alexa · Off → 0x30/0x31/0x32/0xFF), Reset (unavailable, "Phase 4"). App column: Open at login (Toggle → settings), Show in menu bar (on, unavailable), Hide Dock icon (Toggle → settings), Hotkeys (unavailable, "Phase 4"), Battery history (unavailable, "Phase 4"), Appearance (Boxes Paper · Ink · System → settings), Advanced (unavailable, "Phase 4"); footer link list: App version (from `tauri.conf.json` via `getVersion()`), Protocol `MDR v2 · tables 1+2`, "Not affiliated with Sony".
  - **Scenes**: the two placeholder cards from the mockup rendered static with a one-line note "Modes arrive after Phase 2".
- [ ] **Step 2: `pnpm check` green;** run with `--fake-device`, walk every screen, describe what each control did (pending → confirmed) in the report.
- [ ] **Step 3: Commit** `Add the Overview, Noise, Devices, Settings and Scenes screens`.

---

### Task 6: Sound screen with draggable faders

**Files:**
- Create: `src/lib/controls/Faders.svelte`, `src/views/Sound.svelte`
- Test: `src/lib/controls/Faders.test.ts`

**Interfaces:**
- `Faders.svelte` props: `bands: number[] | null` (raw 0..20, centre 10), `labels: string[]` (`Clear Bass`, `400 Hz`, `1 kHz`, `2.5 kHz`, `6.3 kHz`, `16 kHz`), `status`, `disabled`, `oncommit: (bands: number[]) => void`; shows ±10 dB marks, per-band value label (`+4`, `0`, `-1`), handle follows pointer, keyboard ↑/↓ steps by one, commit once on pointer release or 150 ms after the last key.
- `Sound.svelte`: faders (`set_eq_bands`), preset grid (`set_eq_preset`; names from `preset_name`; "Manual" = 0xA0, "Custom 1/2" = 0xA1/0xA2), buttons "Save as Custom 1" (unavailable until Phase 4; needs the device's save semantics) and "Reset" (sets all bands to 10 via `set_eq_bands`), ULT boxes, Codec box (codec in Doto; "Negotiated with This Mac"; Live tag), Connection quality Boxes (Sound quality = 0 · Stable connection = 1 → `set_connection_mode`; **verify** mapping on hardware), DSEE toggles (`dsee` read-only indicator + `set_dsee_auto_off`).

- [ ] **Step 1: Failing tests** — `faders_commit_once_on_release` (simulate pointerdown/move×5/up → `oncommit` called once with the final bands), `faders_keyboard_step_commits_after_delay` (ArrowUp twice → one commit after 150 ms with +2).
- [ ] **Step 2: Run** `pnpm test` → fails. **Step 3: Implement.** **Step 4: `pnpm test && pnpm check`** green; `--fake-device` walkthrough described.
- [ ] **Step 5: Commit** `Add the Sound screen with draggable faders`.

---

### Task 7: Hardware validation and docs (with Roy)

**Files:** Modify `docs/hardware-validation.md` (new section 6), `docs/protocol-notes.md` (resolve the **verify** items touched), `README.md` (Status: Phase 2).

- [ ] **Step 1 (read-only):** run the app, open every screen, confirm every field shows the device's values (compare with section 2 of the validation doc).
- [ ] **Step 2 (writes, owner's go-ahead, originals recorded first and restored after):** EQ preset Bass Boost then back to Custom 2 (`58 03`; if the preset byte is ignored, retry with `58 00` and record which one the device honours); one band change; volume; DSEE auto-off; connection quality; auto power-off; wear pause; touch panel; sidetone; voice guide volume; voice assistant; multipoint on/off (does the `98 00 06 01` tail get an ACK? does `D9 D1` arrive without it?).
- [ ] **Step 3:** themes (Paper/Ink/System), Dock icon behaviour, launch at login (check System Settings › Login Items), window min size.
- [ ] **Step 4:** write the results, commit `Record Phase 2 hardware validation`, push.
