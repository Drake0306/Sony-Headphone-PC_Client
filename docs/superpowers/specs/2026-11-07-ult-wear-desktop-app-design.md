# ULT WEAR desktop app and MDR protocol library — design

Date: 2026-11-07. Author: Roy. Status: approved to build; stack choice is Claude's call and is flagged below.

## Intent

Roy owns a Sony ULT WEAR (WH-ULT900N, firmware 2.1.0) and wants a desktop app that does what Sony's phone app does, looks better, and goes further. The app lives in the macOS menu bar (and the Windows tray) for the daily controls, and opens a full window for everything else. Underneath it is a new, permissively licensed protocol library for Sony's MDR v2 Bluetooth protocol that Roy owns and can extend past what Sony's app exposes.

Why now: Sony ships no desktop app and no SDK. The best existing clients (mos9527, Sony Device Center, SonyBridge) are either ImGui utilities or dark-card dashboards, none exposes the ULT button, none shows confirmed-vs-pending state on controls, and none ships a store-quality, signed build. The protocol is a solved problem (libmdr's extracted tables, Gadgetbridge's verified layouts); the unsolved problems are UX, automation and polish.

Background research lives in `docs/research/` (reports as HTML, notes as Markdown). The approved visual design is `docs/research/reports/Hairline design screens.html`.

## Goals

1. Control the ULT WEAR from a menu-bar popover: noise control (NC / Ambient / Off), ambient level 1–20, Focus on Voice, ULT (Off / 1 / 2), battery, which paired device is playing and a one-click "take audio".
2. A full window with six screens: Overview, Sound (EQ with faders, presets, Clear Bass, codec and connection quality, DSEE), Noise, Devices (multipoint, paired list, pairing mode), Scenes (host-side automation), Settings (headphone settings the device declares, app settings).
3. Every control shows its true state: confirmed (device replied), pending (in flight, striped), refused (orange outline plus reason), unavailable on this model (faded, decided by the capability list).
4. The protocol library is reusable on its own: framing, typed commands, a sans-IO session, platform transports. It is capability-gated (asks the device what it supports) rather than model-gated, so other v2 Sony models work best-effort without code changes.
5. Runs on macOS first, Windows second, Linux third, with the same visual design everywhere.
6. Room to grow into things Sony never shipped: scenes with desktop triggers, hotkeys, a CLI, battery history, probing undeclared opcodes, a frame log for research.

## Non-goals (for now)

- Firmware updates, voice-pack installation, anything over the Airoha RACE channel. Separate transport, real brick risk, later module if ever.
- LE Audio / BLE GATT control path. The ULT WEAR does not advertise LE Audio.
- Speak-to-Chat, Find Your Headphones, 360 Reality Audio setup, head-tracking configuration. Not features of this model (first two) or not worth the surface yet (last two).
- Code signing, notarization, store distribution. Wanted eventually; not part of the first builds.
- v1 protocol (WH-1000XM3/XM4 era). The library is structured so v1 could be added, but nothing is built for it.
- A mobile app.

## Anti-goals (what would make this a failure even if it works)

- Controls that silently snap back because the device refused or the app guessed. Every write is gated on the capability list and confirmed by the device's reply or notify.
- A hero image that pushes controls below the fold. The Overview is complete without scrolling at 880×560.
- Promotion, accounts, telemetry, large installs. Target well under 30 MB and no network access except what Bluetooth needs.
- Bricking or crashing headphones. Hard deny-list for destructive opcodes, read-modify-write for EQ, no writes to undeclared capabilities without an explicit "Advanced" override.
- Anything named "Sony" or "Sound Connect" in the product name. Working codename: **Hairline** (Roy may rename).
- Any Claude attribution in commits, files or metadata.

## Constraints

- Hardware on hand: one WH-ULT900N, firmware 2.1.0, Forest Gray. Everything else is unverified.
- Protocol facts (see `docs/research/notes/Sony headphone PC client comparison/ult_wear_support.md` and `extending_beyond_official_app.md`):
  - MDR v2 only, over Bluetooth Classic RFCOMM, service UUID `956C7B26-D49A-4BA8-B03F-B17D393CB6E2`, channel discovered via SDP (18 on this unit; never hard-coded).
  - Frame: `0x3E | type | seq | len u32 BE | payload | checksum | 0x3C`; bytes 0x3C/0x3D/0x3E in the body escaped as `0x3D, b & 0xEF`; checksum = 8-bit sum excluding the 0x3E header; types ACK 0x01, COMMAND_1 0x0C, COMMAND_2 0x0E; stop-and-wait ARQ, one frame in flight, ACK carries the flipped sequence number; the device retransmits forever if a frame is not ACKed.
  - Init `00 00` on table 1 → reply length 8 means v2; protocol info `01 00 03 00 20 02 00 00` (tables 1 and 2).
  - Capability discovery: `06 00` on both tables → `07 00 <n> (<function> <priority>)×n`. This unit's table 1: `10 12 13 14 15 20 23 25 27 32 53 69 6B 70 93 F4 90 A1 F1 B2 B7 C1 C2 D1 D2 D3 E3 E2 F9 FD`; table 2: `42 50 F2`.
  - ULT = EQ inquired type 0x03: GET `56 03` → `57 03 <preset> <ult> 06 <b1..b6>`; SET `58 03 …` → NTFY `59 03 …`; ult 00/01/02; bands centred at 0x0A with 21 steps; capability `50 03` → `51 03 06 15 0A 0C (<id><name>)…`. Whether type 03 honours the preset byte is disputed between two hardware reports; the builder tests both `58 03` and `58 00` on Roy's unit and documents the answer.
  - NC/ambient: `68 17 <valueChange> <total> <mode 00 NC / 01 ASM> <asmMode 00 normal / 01 voice> <level 00..14>` (7-byte layout; the 8-byte layout is ignored by this model); ambient fields ignored while in NC.
  - Battery `22 00` → `23 00 <level> <charging>`, no threshold byte.
  - Volume `A6 20` / `A8 20` on a 0–30 scale.
  - Multipoint on/off: GET `d6 d1` → `d7 d1 00 XX` (00 = on), SET `d8 d1 00 XX` then `98 00 06 01`.
  - Only one control connection at a time; the phone app and the desktop app compete.
  - Destructive opcodes: `F8` (factory reset on v2), `98` (reboot family), `24` (power off), `2D` (firmware transfer), `C4` (JSON dump). Never sent except power-off and reset behind explicit confirmation.
- Licenses: command tables are ported from mos9527/SonyHeadphonesClient `libmdr` (MIT, attribution kept in `THIRD_PARTY.md`). Gadgetbridge (AGPL-3.0) and Resonance (PolyForm NC) are read for behaviour only; no code is copied from them. The project's own license: MIT.
- Git: personal account Drake0306 via per-repo `core.sshCommand` with `~/.ssh/id_github`; remote `git@github.com:Drake0306/Sony-Headphone-PC_Client.git`; repo is public.

## Decisions

### Stack (assistant's call, flagged for veto)

Earlier in the conversation Claude proposed native SwiftUI on Mac plus a separate WinUI app. The chosen design (Hairline) uses no system materials and must look identical on all three platforms, Roy wants one new library he owns, and one person is building. So:

- **Core: Rust workspace.** Crates `mdr-frame` (framing, escaping, checksum, ARQ; `no_std` + alloc), `mdr-proto` (typed opcodes, inquired types, payload codecs; data-driven so a new opcode is an enum entry plus a codec), `mdr-session` (sans-IO state machine: handshake, capability discovery on both tables, request/reply correlation, state cache, notify dispatch, deny-list and capability gating), `mdr-transport` (platform RFCOMM: macOS via `objc2-io-bluetooth`, Windows via `windows` crate Winsock `AF_BTH`/`BTHPROTO_RFCOMM` with `serviceClassId`, Linux via `bluer`; plus a scripted fake transport for tests and UI work), `mdr-cli` (later).
- **App: Tauri 2 + Svelte 5 + TypeScript + Vite.** The Rust side of the Tauri app owns the single Bluetooth connection (the broker) and exposes commands and events over Tauri IPC. The frontend is the Hairline mockups made real: same tokens, same controls. Fonts (Instrument Sans, Doto, DM Mono) are bundled, not fetched.
- **Menu bar / tray.** Tauri tray icon; popover is a Tauri window turned into a non-activating `NSPanel` on macOS (`tauri-nspanel`) positioned under the icon (`tauri-plugin-positioner`); on Windows a borderless flyout near the tray; on Linux a StatusNotifierItem. Dock icon hidden by default (Accessory activation policy), shown while the main window is open. Launch at login via `tauri-plugin-autostart`. Hotkeys via `tauri-plugin-global-shortcut`.
- **Why not SwiftUI:** two UIs for one design, and the design does not benefit from native materials. **Why not Electron:** size. **Why Rust over C++ (vendoring libmdr directly):** no FFI into the app, a cleaner sans-IO layering (overheard's shape), and Roy owns the result. The cost is porting the tables rather than linking them.

### Architecture

```
┌──────────────────────── Tauri app (one process) ────────────────────────┐
│  Svelte UI (popover window, main window)                                │
│      ▲ events (state, connection, battery…)   │ commands (set_nc…)       │
│  Rust broker task: owns MdrSession + transport, serialises writes,      │
│  applies capability gating + deny-list, emits state diffs,              │
│  runs Scenes engine (host-side triggers → commands),                    │
│  local JSON socket for CLI/hotkeys (phase 4)                            │
│      ▲                                                                   │
│  mdr-session (sans-IO)  ←→  mdr-transport (RFCOMM per platform)         │
│      ▲                                                                   │
│  mdr-proto (codecs)  ←  mdr-frame (bytes)                               │
└──────────────────────────────────────────────────────────────────────────┘
```

- The broker is the only thing that talks to the headphones. UI never sends bytes.
- State model: a single `DeviceState` struct (connection, battery, nc mode, ambient level, voice focus, ult, eq preset and bands, dsee, connection mode, multipoint on/off, paired devices with playing flag, capability list, model, firmware). Each field carries `Confirmed | Pending(since) | Refused(reason) | Unavailable`.
- Writes: UI calls a command → broker checks capability and deny-list → marks field Pending → sends → on RET/NTFY marks Confirmed with the device's value → after 1.5 s without reply marks Refused("no reply") and re-queries.
- Reconnect: the broker polls paired devices for the v2 service and connects when the ULT WEAR appears; it backs off while "pairing mode" is requested and while the phone app holds the channel (EBUSY / connect failure).
- Scenes (phase 4): first-match-wins rules; triggers available on macOS first: a paired device started playing (from device notify), an app holds the microphone (CoreAudio), time of day, Wi-Fi SSID. Actions are ordinary commands plus "take audio".

### Model support

- Primary: WH-ULT900N. Name match `ULT WEAR` or `Sony ULT`, and SDP presence of the v2 UUID.
- Secondary: any device advertising the v2 UUID. Everything is driven by the `06/07` capability list, so other v2 models get whatever they declare. Nothing is promised for them.

### UI

- Exactly the screens in `Hairline design screens.html`: popover (connected / not connected), Overview, Sound, Noise, Devices, Scenes, Settings, plus the four control states. Window 880×560 minimum; popover 340 wide.
- Tokens: paper `#efede6`, ink `#161614`, forest `#4a5550`, hairline `#cfccc2`, muted `#6f726d`, ULT orange `#ff6600` (used only for the active ULT level and "playing"), box `#f8f7f3`. Type: Doto 900 for numerals, Instrument Sans for text, DM Mono for labels. Themes: Paper (default), Ink (inverted, forest stays), System.
- Controls imported from the Panel direction: boxed switches, Link list, faders.
- Reduced motion respected; keyboard focus visible; hotkeys configurable.

### Testing

- `mdr-frame`: unit tests with the verified byte strings from the research notes (handshake, battery `23 00 43 00`, NC/ambient `68 17 …`, ULT `57 03 00 02 06 0A×6`, escape case `0x3E80`).
- `mdr-session`: scripted conversations through the fake transport (connect → capability lists → state reads; set NC → ACK → NTFY; refused write; no-reply timeout).
- App: the fake transport drives the UI in a `--fake-device` mode so screens can be built without headphones; a `--record` flag writes a scrubbed frame log for fixtures (device addresses and names replaced).
- Hardware checks on Roy's unit are an explicit step at the end of each phase, with results written to `docs/hardware-validation.md`.

### Phases

1. **Core and popover on macOS.** Workspace, `mdr-frame`, `mdr-proto` (ULT WEAR subset: init, protocol info, capability discovery, device info, battery, NCASM 0x17, EQ/ULT types 00 and 03, multipoint, volume, power off), `mdr-session`, macOS transport, Tauri app with tray + popover showing live battery, NC/AMB/OFF, level, ULT, playing device. Hardware validation.
2. **Main window.** Overview, Sound, Noise, Devices, Settings screens with real state; DSEE, connection mode, auto power-off, wear pause, touch panel, voice guide, Quick Access, voice assistant via general settings; control states; Paper/Ink themes; launch at login; hide Dock icon.
3. **Windows.** Winsock transport, tray flyout, same UI. Hardware validation on a Windows machine when Roy has one available; otherwise built and unit-tested only.
4. **Beyond Sony.** Scenes with desktop triggers, global hotkeys, local socket + `mdr-cli`, battery history, Advanced panel (frame log, capability list, raw command behind the deny-list), opcode probe mode that walks GET-only commands and records which undeclared ones answer.
5. **Linux.** `bluer` transport, StatusNotifierItem tray, packaging later.

## Left to the builder

- Internal module layout inside each crate, naming of Tauri commands and events, Svelte component breakdown.
- Verifying every opcode and payload layout against the libmdr generated headers and the research notes before use; recording discrepancies in `docs/protocol-notes.md`.
- Choice of SDP lookup method per platform (UUID-based connect where the OS supports it, explicit SDP query otherwise).
- How the popover is positioned and dismissed on each platform within the constraints above.
- Exact timeout values, reconnect back-off, and the fake device's scripted behaviour.
- Font licensing files and bundling method (OFL for all three faces).

## Builder-check resolutions (2026-11-07)

A fresh reviewer read this document and asked five questions. Resolutions:

1. **Paired devices, "playing", "take audio".** The ULT WEAR declares no peripheral-management function on table 2, so on this model these are host-side: "playing" = this computer is the active A2DP source for the headphones (plus `A3 01` playback status when it answers); "take audio" = ask the OS to make this computer the active audio source (macOS: connect/activate the device as output; later platforms likewise). Phase 1 ships the popover's device row on that basis and does not block on the headphones. Phase 1 starts (not ends) with a read-only hardware spike that probes the table-2 peripheral GETs (`36 02`, `32 …`) and records what answers in `docs/hardware-validation.md`. Capability gating in `mdr-session` is "declared by `06/07`, or proven by a read-only probe on this unit" (a per-unit allowlist stored with the device record). SETs on proven-but-undeclared commands stay behind the Advanced panel.
2. **Deny-list granularity.** Keyed on opcode plus inquired sub-id. `98 00 16 …` (reboot) and the other destructive entries are denied; `98 00 06 01` is allowed only as the tail of the multipoint-toggle sequence, which is therefore an ordinary control, not a confirmed action. Power off (`24 03 01`) and reset (`F8 09 …`) stay behind explicit confirmation.
3. **Codec box values.** Codec comes from the wire (`12 02` / `13 02`) in Phase 1. Sample rate, bitrate and "negotiated with" come from the macOS audio stack (CoreAudio / IORegistry) in Phase 2, macOS only; Windows and Linux show blank until a platform module exists. "Time left" needs battery history (Phase 4) and shows blank before that. Blank, never invented.
4. **Hero image.** Our own illustration (the SVG silhouette from the mockups, tinted by the reported colourway), shipped in the repo under MIT. No Sony renders. The Overview layout must hold without the image.
5. **Commit attribution.** Encoded in `CLAUDE.md` at the repo root: no Co-Authored-By or "Generated with" lines, no Claude credits anywhere in the repo. Commits are authored by Roy's identity only.
