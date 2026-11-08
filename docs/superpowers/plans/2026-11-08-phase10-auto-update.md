# Phase 10: In-app updates from GitHub Releases — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Hairline notices a newer GitHub release, offers "Update now", downloads and installs it in the background with a progress bar, and relaunches as the new version, on macOS and Windows (Linux follows the same path with an AppImage, lower priority).

**Architecture:** `tauri-plugin-updater` reads `latest.json` from the latest GitHub release (published by `tauri-action` with `includeUpdaterJson: true`), verifies each download against the public key below (minisign; the private key is a GitHub Actions secret), installs the platform bundle and `tauri-plugin-process` relaunches. A small `updater.rs` in the app wraps the plugin: a check on launch (after 15 s) and every 6 hours while running, a manual check, a download with progress events, and an install step. The frontend shows the state in Settings and a one-line notice in the popover footer. Nothing about the headphones changes.

**Public key (built into the app):** `dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IDFCMDk2QTVGMzJDQ0VENkUKUldSdTdjd3lYMm9KR3dQY3ZFUWVUelpkZDdpQ3FwRk44MHFjcDMvd25ZMkZMcmVuK2N4SVZNbUQK`

**Feed URL:** `https://github.com/Drake0306/Sony-Headphone-PC_Client/releases/latest/download/latest.json`

**Spec:** owner's request of 2026-11-08 (update notice, Update now, silent install with any needed permission prompt, close and restart as the new version; macOS and Windows first).

## Global Constraints

- No AI attribution; plain commits; full acceptance set (cargo test/clippy/fmt, Windows cross-check, `pnpm check`, `pnpm test` exit 0).
- The app never installs anything the user did not click Update now for; the only automatic step is the check. A failed signature check is shown as "Update could not be verified" and nothing is installed.
- Nothing static: the version shown is the running app's (`app.package_info().version`); the available version, notes and date come from `latest.json`.
- Secrets never enter the repo: the private key (no password) lives in the Actions secret `TAURI_SIGNING_PRIVATE_KEY` and in the owner's `~/.tauri/hairline.key`.
- Version bump to 0.2.0 in `tauri.conf.json`, `package.json` and the app's `Cargo.toml` in this phase; the release tag is `v0.2.0`.

## Review Focus

1. A tampered or unsigned download must never be installed — Task 1 `update_without_valid_signature_is_refused` (unit test on the state mapping of the plugin's error; the plugin itself enforces the signature).
2. The notice must not nag: once dismissed with Later, the same version is not shown again until the next launch — Task 2 `later_hides_that_version_until_relaunch`.
3. An update check failing (offline) must show nothing and log once — Task 1 `offline_check_is_silent`.
4. The progress bar must reach 100 % before "Installing…" and the relaunch call must happen only after install returns Ok — Task 1 `install_then_relaunch_order` (pure sequencing helper).
5. Auto-check can be switched off in Settings and the switch is honoured on the next launch — Task 2 `auto_check_setting_round_trip`.

---

### Task 1: Plugin, config, backend
**Files:** `apps/desktop/src-tauri/Cargo.toml` (+ `tauri-plugin-updater = "2"`, `tauri-plugin-process = "2"`), `tauri.conf.json` (`plugins.updater.endpoints`, `plugins.updater.pubkey`, `bundle.createUpdaterArtifacts: true`, version 0.2.0), `capabilities/default.json` (`updater:default`, `process:allow-restart`), `package.json` version, create `src/updater.rs`, modify `src/{lib,settings,commands}.rs`, tests `tests/updater.rs`.
**Interfaces:** commands `check_update() -> Option<UpdateInfo { version, notes, date }>`, `install_update() -> Result<(), String>` (downloads with `"update"` events `{ phase: "downloading"|"installing"|"restarting"|"failed", downloaded, total, error }`, installs, relaunches), `current_version() -> String`; settings field `auto_check_updates: bool` (serde default true); background check task spawned in `setup` (15 s after launch, then every 6 h) emitting `"update"` `{ phase: "available", version, notes, date }` when newer; errors `eprintln!` once and surface as `phase: "failed"` only for a user-initiated check/install.
- [ ] Red → green → full checks. Commit `Add in-app updates from GitHub releases`.

### Task 2: Settings and popover UI
**Files:** `src/views/Settings.svelte` ("Updates" block: "Hairline {version}" · Check now; when available: "Hairline {v} is available" with notes, Update now / Later; progress bar during download; "Installing…"/"Restarting…"; failure note), `src/views/Popover.svelte` (footer line "Update available · open Hairline" when available and not dismissed), `src/lib/update.ts` (store fed by the `"update"` event and `check_update`; `dismissed` kept in memory only), `src/lib/ipc.ts`, tests.
- [ ] Commit `Show updates in Settings and the popover`.

### Task 3: Release workflow and docs
**Files:** `.github/workflows/release.yml` (`includeUpdaterJson: true`, env `TAURI_SIGNING_PRIVATE_KEY` from the secret, keep both platforms), README ("Updates" section: how the check works, the Later/Update now flow, that updates are signed, how to switch auto-check off), `.github/release-notes.md` (one line: Hairline now updates itself from this page).
- [ ] Commit `Sign and publish updater artifacts from the release workflow`.

### Task 4 (controller): tag `v0.2.0`; after that release is live, bump to 0.2.1 with a notes-only change, tag `v0.2.1`, and confirm `latest.json` lists 0.2.1 so the installed 0.2.0 offers the update.
