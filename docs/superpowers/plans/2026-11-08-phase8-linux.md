# Phase 8a: Linux Transport and App Build — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `mdr-transport` discovers paired Sony headphones and opens the MDR v2 RFCOMM channel on Linux, and the Hairline app compiles, passes its tests and starts on Linux with a tray icon and the plain popover window from Phase 7a. No Linux machine with the headphones is available, so nothing is claimed as run against hardware; a Linux CI job builds the crates and the app.

**Architecture:** A `linux` module in `crates/mdr-transport` mirrors `windows.rs`: discovery through BlueZ over D-Bus with the `bluer` crate (adapter → paired devices → name, address, `connected`), and the RFCOMM link through a plain `libc` `AF_BLUETOOTH`/`BTPROTO_RFCOMM` socket (`sockaddr_rc`), synchronous like the other transports, with `SO_RCVTIMEO` for `recv_timeout`. BlueZ no longer exposes SDP browsing over D-Bus, so the channel comes from `HAIRLINE_RFCOMM_CHANNEL` when set, else the channel recorded in the device record (`devices/<addr>.json`, new optional `rfcomm_channel` field written after a successful connect), else a bounded scan of channels 1..=30 that stops at the first channel which connects and answers the protocol-info GET `00 00` with a v2 reply (any other channel is closed immediately; nothing but that one GET is ever sent while scanning). `discover.rs` dispatches under `cfg(target_os = "linux")`. The app gains `cfg(target_os = "linux")` stubs where Windows has them (sensors: frontmost app `None`, mic `false`; audio devices empty) and a tray that uses Tauri's Linux tray (libayatana-appindicator).

**Tech Stack:** `bluer` (default features minus `rfcomm`; D-Bus only), `libc`, a small `tokio` current-thread runtime for the `bluer` calls (bluer is async). CI: `ubuntu-latest` with `libdbus-1-dev`, `libwebkit2gtk-4.1-dev`, `libayatana-appindicator3-dev`, `librsvg2-dev`, `patchelf`.

**Spec:** `docs/superpowers/specs/2026-11-07-ult-wear-desktop-app-design.md` — Core: "Linux via `bluer`"; Phases "5. Linux. `bluer` transport, StatusNotifierItem tray, packaging later."

## Global Constraints

- No AI attribution; plain commits; every commit passes the macOS acceptance set plus `cargo check -p mdr-transport -p mdr-cli --target x86_64-unknown-linux-gnu` (install the target; if `bluer`/`libc` cross-check fails for reasons outside our code, record it and rely on the CI job).
- macOS and Windows behaviour unchanged.
- The channel scan sends exactly one frame per candidate channel (`00 00` protocol-info GET through the normal framing) and closes the socket otherwise; the deny-list is irrelevant to it but the scan code must not be able to send anything else (pinned by a test on the pure "what to send" helper).
- Nothing static; blanks where Linux has no module.
- Docs claim only "builds and passes tests on Linux CI; not run against the headphones on Linux".

## Review Focus

1. The channel scan must never send anything but the protocol-info GET — Task 1 `scan_sends_only_protocol_info_get`.
2. A paired-but-disconnected device yields `connected_audio: false` and `connect` fails with `NotConnected` without opening a socket — Task 1 `connect_refuses_unconnected_candidate`.
3. `sockaddr_rc` layout (family u16, bdaddr 6 bytes little-endian, channel u8) and address byte order must match BlueZ: `A8:E6:E8:FE:D6:54` → bytes `[0x54,0xD6,0xFE,0xE8,0xE6,0xA8]` — Task 1 `bdaddr_is_little_endian`.
4. The recorded channel must be used before any scan on the next connect — Task 1 `recorded_channel_skips_the_scan` (pure planner test).
5. The Linux CI job must build the app, not only the crates — Task 2.

---

### Task 1: Linux transport module

**Files:** create `crates/mdr-transport/src/linux.rs`; modify `crates/mdr-transport/{Cargo.toml,src/lib.rs,src/discover.rs}`; `apps/desktop/src-tauri/src/device_record.rs` (optional `rfcomm_channel: Option<u8>`, serde default) and the broker's connect path (store the channel after connect; pass it to `connect` via a new `connect_with_hint(address, channel: Option<u8>)` on the transport crate, default dispatch for other platforms ignores the hint).
**Tests:** pure helpers on every OS (`bdaddr_is_little_endian`, `scan_sends_only_protocol_info_get`, `channel_plan_order` (env → record → scan 1..=30), `recorded_channel_skips_the_scan`, `connect_refuses_unconnected_candidate`, `linux_error_mapping`); socket code `#[cfg(target_os = "linux")]`.
- [ ] Red → implement → green + cross-check. Commit `Add the Linux RFCOMM transport`.

### Task 2: Linux app build and CI

**Files:** `apps/desktop/src-tauri/src/{lib,popover,broker,sensors_*,audio_*}.rs` (Linux arms reuse the Windows plain-window popover and stubs), `.github/workflows/ci.yml` (`linux-app` job: apt deps, pnpm build, `cargo test --workspace`, clippy), README Status + `docs/hardware-validation.md` §11 Linux checklist.
- [ ] Commit `Build and test the app on Linux in CI`.
