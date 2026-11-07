# Phase 7a: Windows App Build — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The Hairline desktop app compiles, passes its tests and starts on Windows: a tray icon with a borderless always-on-top popover window beside the tray in place of the macOS panel, the Winsock transport from Phase 3a, honest blanks where macOS-only sensors have no Windows module yet, and a Windows CI job that builds the whole app. No Windows hardware is available, so nothing is claimed as run against the headphones.

**Architecture:** Keep one code base. Every `#[cfg(target_os = "macos")]` branch gets a Windows counterpart or an explicit stub that returns "unknown" (never a guess): popover (plain `WebviewWindow` positioned with `tauri-plugin-positioner` `TrayBottomRight`, hidden on focus loss, no Dock/activation-policy code), audio "playing" detection (stub: `devices` empty, so the Link list shows nothing rather than a wrong "playing"), sensors (frontmost app via `GetForegroundWindow` + `GetWindowThreadProcessId` + `QueryFullProcessImageNameW`; mic-in-use stub `false` until a WASAPI session check lands), capture directory via Tauri's path API (already cross-platform), the socket (already `cfg(unix)`-gated; Windows prints "not supported"). CI gains a `windows-app` job: pnpm install, `pnpm build`, `cargo test -p hairline`, `cargo clippy -p hairline --all-targets -- -D warnings`.

**Tech Stack:** as before, plus `windows-sys` features `Win32_UI_WindowsAndMessaging`, `Win32_System_Threading` for the frontmost-app sensor.

**Spec:** `docs/superpowers/specs/2026-11-07-ult-wear-desktop-app-design.md` — Phases "3. Windows. Winsock transport, tray flyout, same UI. Hardware validation on a Windows machine when Roy has one available; otherwise built and unit-tested only."; Menu bar / tray: "on Windows a borderless flyout near the tray". Owner's platform order: macOS, Windows, Linux.

## Global Constraints

- No AI attribution; plain commits; every commit passes the macOS acceptance set AND `cargo check -p hairline --target x86_64-pc-windows-msvc` if that cross-check is possible from macOS (Tauri's build script may need the frontend `dist`; if cross-check is impossible, say so in the report and rely on the new CI job).
- macOS behaviour is unchanged: no macOS code path edited except to move it behind the same `cfg` it already has.
- Nothing static: a value a Windows module cannot provide is blank/empty, never a placeholder. The FAKE label logic is unchanged.
- The deny-list and gating are untouched (no transport or session change in this phase).
- README and docs claim only "builds and passes tests on Windows CI; not run against the headphones on Windows".

## Review Focus

1. The popover must hide when it loses focus on Windows the same way it does on macOS, without the macOS activation-policy code — Task 1 `popover_hides_on_focus_loss` (unit test on the pure decision helper, not a GUI test).
2. The Link list must show no "playing" on Windows rather than a wrong one — Task 1 `windows_paired_devices_are_empty` (stub returns an empty list; the frontend already renders nothing for an empty list).
3. The frontmost-app sensor must return `None` on any failure, never panic — Task 1 `frontmost_app_none_on_failure` (pure mapping helper).
4. The Windows CI job must build the app, not only the crates — Task 2 checks the workflow runs `pnpm build` before `cargo test -p hairline`.
5. A `HAIRLINE_FAKE=1` start on Windows must reach the popover with the FAKE label (no macOS-only call on the startup path) — Task 1 `startup_path_has_no_macos_only_calls` is a code-review item rather than a test; the CI job's `cargo test -p hairline` runs the broker and settings tests on Windows.

---

### Task 1: Windows code paths in the app

**Files:**
- Modify: `apps/desktop/src-tauri/src/{popover,dock,lib,broker,sensors_macos,audio_macos}.rs`, `Cargo.toml` (`[target.'cfg(windows)'.dependencies] windows-sys` features)
- Create: `apps/desktop/src-tauri/src/sensors_windows.rs`, `src/audio_windows.rs` (stub with a doc comment naming the WASAPI route for later)
- Test: `apps/desktop/src-tauri/tests/windows_paths.rs` (pure helpers, run on every OS) plus `#[cfg(windows)]` tests that run in CI only

- [ ] Popover on Windows: `WebviewWindowBuilder` with `decorations(false)`, `always_on_top(true)`, `skip_taskbar(true)`, positioned `Position::TrayBottomRight` on tray click, hidden on `WindowEvent::Focused(false)` with the same 300 ms reopen guard; the main window opens normally (no Dock policy). Factor the "should hide now?" decision into a pure helper used by both platforms and test it.
- [ ] `audio_windows.rs`: `paired_devices()` → empty, `output_device_name()` → None, with a doc comment; `broker.rs` dispatches by `cfg`.
- [ ] `sensors_windows.rs`: `frontmost_app_name()` via `GetForegroundWindow`/`GetWindowThreadProcessId`/`OpenProcess`/`QueryFullProcessImageNameW` → the executable's file stem (e.g. "Zoom"); `mic_in_use()` → `false` with a comment; failures → `None`/`false`, logged once.
- [ ] `lib.rs`/`dock.rs`: compile on Windows without the macOS-only plugin and policy calls (already partly gated; make it complete).
- [ ] Commit `Build the app on Windows with a plain popover window`.

---

### Task 2: Windows app CI job and docs

- [ ] `.github/workflows/ci.yml`: job `windows-app` on `windows-latest`: checkout, rust toolchain (clippy), rust-cache, pnpm 10 + Node 22, `pnpm install --frozen-lockfile` in `apps/desktop`, `pnpm build`, `cargo test -p hairline`, `cargo clippy -p hairline --all-targets -- -D warnings`. Keep `windows-crates` as is or fold it in.
- [ ] README Status: "Phase 7a: the app builds and its tests pass on Windows CI. The tray popover is a plain window there; audio 'playing' and the microphone trigger are blank until Windows modules exist. Not run against the headphones on Windows." `docs/hardware-validation.md` §10 Windows app checklist (to do with a Windows machine).
- [ ] Commit `Build and test the app on Windows in CI`.
