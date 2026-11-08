# Session 2 recap — SHDC (Sony Headphone Desktop Client)

Written 9 November 2026 for a future Claude session. Read this first when Roy says
"look at your last session" or "pick up where session 2 left off". Session 1 was the
research and Phase 1 (6–7 November 2026); this file covers everything up to v0.5.2.
Internal note, so Markdown; reports for Roy are HTML (see `CLAUDE.md`).

## 1. What this is

- **SHDC** is Roy's own desktop app and protocol library for Sony headphones, built on the
  Hairline design language. Roy owns a Sony ULT WEAR WH-ULT900N (fw 2.1.4, Black); it is the
  only unit the app has been tested against. Repo: `Drake0306/Sony-Headphone-PC_Client`.
- The app was called **Hairline** until v0.3.0. "Hairline" now names only the design language
  (tokens, `docs/research/reports/Hairline design screens.html`). The bundle identifier stays
  `com.drake0306.hairline` on purpose (settings dir, log dir, updater continuity).
- Platform priority: macOS first, then Windows, then Linux. Roy tests on a Mac and a Windows
  machine. No Linux hardware so far.
- Rules Roy set, in his words or close to them: no AI attribution anywhere in commits or files;
  personal git identity per repo (`Drake0306`, `~/.ssh/id_github`); never send deny-listed bytes
  to the headphones (`docs/protocol-notes.md`); never SET an undeclared capability outside the
  Advanced panel flow; "nothing static, everything dynamic"; no reset-headphones feature anywhere;
  release notes and docs in plain, direct language; deliverables for him are HTML.

## 2. Stack and layout

- Rust workspace: `crates/mdr-frame`, `mdr-proto`, `mdr-session`, `mdr-transport` (macOS IOBluetooth,
  Windows Winsock RFCOMM, Linux BlueZ/D-Bus + libc socket, fake device), `crates/mdr-cli`
  (binary `shdcctl`, talks to the app over a local socket `shdc.sock`).
- App: `apps/desktop` — Tauri 2.12 (`src-tauri`, lib `shdc_lib`, binary `shdc`) + Svelte 5 runes
  + Vitest. Tokens in `src/styles/tokens.css`; a script forbids hex colours anywhere else.
- Protocol facts live in `docs/protocol-notes.md`; hardware results and checklists in
  `docs/hardware-validation.md`; plans in `docs/superpowers/plans/`; research reports (HTML) in
  `docs/research/reports/`; status pages in `docs/status/`.
- Process used: Superpowers subagent-driven development per phase (plan → implementer →
  reviewer → fix rounds → whole-branch review → merge to main), ledgers under
  `.superpowers/sdd/`. Later small features were done inline with one or two independent
  reviewer subagents before merge. Every merge ran: cargo fmt/clippy/test, `pnpm check`,
  `pnpm test`; CI has macOS, windows-crates, windows-app and linux-app jobs.

## 3. What shipped (all on `main`, all released)

| Release | Date | Contents |
|---|---|---|
| Phases 1–2b (pre-release) | 6–8 Nov | Protocol crates, menu-bar popover (NSPanel), full main window (Overview, Sound, Noise, Devices, Settings), Paper/Ink/System themes, launch at login, Scenes (triggers: manual, connect, time, frontmost app, mic in use; hotkeys; ⌥⌘N opens the window). |
| Phase 3a, 4, 5, 5b, 5c, 7a, 8a | 8 Nov | Windows transport; Advanced panel (capability list, frame log, raw command with typed SEND, GET-only opcode probe, "proven" functions unlock GETs only), battery history with time left, local socket + `shdcctl`; redesigned faders (21 segments, zero line); Advanced ten-band equalizer with best fit to the six device bands, confirmation before send, Reset; share codes (15 Crockford symbols + QR) with Paste UI; Windows app build (plain popover, colour tray icon); Linux app build. |
| v0.1.0 | 8 Nov | First GitHub release via `release.yml` (tauri-action). macOS universal dmg, ad-hoc signed; Windows msi + nsis, unsigned. |
| v0.2.0 / v0.2.1 | 8 Nov | In-app updater (`tauri-plugin-updater`, minisign key at `~/.tauri/hairline.key`, no password, public key in `tauri.conf.json`, repo secret `TAURI_SIGNING_PRIVATE_KEY`). 0.2.1 proved the update loop. |
| v0.3.0 | 8 Nov | Rename to SHDC everywhere (window titles, tray, Settings about block with trademark line, installers, logs, env vars `SHDC_*`, socket, CLI `shdcctl`). |
| v0.3.1 | 8 Nov | Updater survives GitHub's release-download host failing (it returned 504 from GitHub's Central India edge for about an hour): second feed on the `updates` branch via raw.githubusercontent.com, download links rewritten to api.github.com by the workflow's `feed` job (`.github/scripts/api-feed.py`), `SHDC_UPDATE_FEED` override, live test `cargo test -p shdc --test updater_live -- --ignored --nocapture`. |
| v0.3.2 | 8 Nov | Windows popover placed from the monitor work area (pure function `windows_position`, tests); "Open window" also while disconnected. |
| v0.4.0 | 8 Nov | Ten palette themes (Latte, Solarized, Dawn, Sepia, Kraft, Nord, Mocha, Gruvbox, Tokyo Night, OLED Black), swatch picker with keyboard navigation, tolerant theme loading (unknown id → Paper, other settings kept). |
| v0.5.0 | 8 Nov | Six flair themes (Riso, Acid, Sunset, Spectrum, Synthwave, Aurora): sixteen extra tokens (wash, accent-2/3, glow, band and fill colours), defaulted on `:root, [data-theme]`. |
| v0.5.1 | 8 Nov | Glass top bar over scrolling screens (color-mix + backdrop blur, hairline once scrolled). |
| v0.5.2 | 8 Nov | Windows white main window fix (async window command + `run_on_main_thread`, hidden until page load, 1.5 s fallback); diagnostics log `shdc.log`; crash notice drawn on the page; tray "Web inspector" with Advanced on. |

Test counts at v0.5.2: about 260 Rust tests, 223 Vitest tests, all green; CI green on all jobs.

## 4. How things work (the parts you will need)

- **Releases**: bump the version in four places (`apps/desktop/package.json`,
  `apps/desktop/src-tauri/tauri.conf.json`, `apps/desktop/src-tauri/Cargo.toml`, then
  `cargo metadata` refreshes `Cargo.lock`), rewrite the "What changed" block of
  `.github/release-notes.md` in plain language, merge to main, `git tag -a vX.Y.Z`, push the tag.
  `release.yml` builds macOS + Windows, then the `feed` job rewrites `latest.json` and pushes the
  `updates` branch. Watch with `gh run watch <id>`.
- `[profile.release] strip = "none"` is required (macOS 26 CLT `strip` breaks dylibs). Local dmg
  bundling fails (Finder AppleEvent timeout); CI works. `cargo check --target x86_64-pc-windows-msvc`
  of the app crate fails locally on `ring`'s C build; CI's windows-app job is the Windows check.
- **The `updates` branch is not to be merged**: it holds only `latest.json`, the fallback feed.
- **Updater**: feeds = release `latest.json`, then raw `updates/latest.json` (raw caches ~5 min,
  sometimes longer). Downloads go through `api.github.com/repos/.../releases/assets/<id>`.
  On macOS the in-app update keeps the installed file name (e.g. `Hairline.app`); the app inside
  is new.
- **Themes**: one `[data-theme="id"]` block per theme in `tokens.css`; `[data-theme=…]` matches
  any element so picker swatches paint themselves; System = no attribute; `theme.test.ts` keeps
  the picker list and the CSS in step; accents must pass about 3:1 on paper and `--on-ult` on
  `--ult` (reviewers caught Kraft orange at 1.6:1 and Latte/Dawn at 2.6:1).
- **Diagnostics** (0.5.2): `shdc.log` in the app log dir (macOS
  `~/Library/Logs/com.drake0306.hairline/`, Windows `%LOCALAPPDATA%\com.drake0306.hairline\logs\`)
  records main-window creation, page loads and page errors; `SHDC_FRAME_LOG=1` writes frames to
  `captures/`; `SHDC_FAKE=1 pnpm tauri dev` runs against the fake device.
- **Capability gating**: typed SETs need a declared function; the probe's "proven" functions only
  unlock GETs. Other Sony v2 models connect safely but NC and EQ show unavailable (decoders are
  ULT WEAR specific).
- **Disk**: Roy's Mac filled up on 9 November; `target/` (Rust build output) and the cargo caches
  were deleted with his permission. They rebuild on the next `cargo` command. Watch disk space
  before long build sessions.

## 5. Pending: fixes awaiting Roy's confirmation

1. **Windows popover position** (v0.3.2). First Windows run put the popover far above the taskbar;
   now placed from the work area. Not yet confirmed on his machine. If still off, ask for: where it
   lands, display scaling percentage, taskbar edge; `shdc:` stderr lines log the computed numbers in
   a dev run.
2. **Windows white main window** (v0.5.2). Cause: synchronous window-creating command ran inside
   the popover's WebView2 callback. Not yet confirmed. If still white: get
   `%LOCALAPPDATA%\com.drake0306.hairline\logs\shdc.log` (does the page load?) and use the tray
   "Web inspector" (Advanced panel on) for the console.
3. **Windows in-app update** has never been observed on hardware.
4. **Themes on Roy's screen**: Spectrum and Sunset knob colours sit just above 3:1; he may want
   them darker. Flair themes were approved from the preview pages, not yet seen in the app by him.
5. **Glass bar** (v0.5.1) not yet seen by Roy.

## 6. Pending: work not started or undecided (Roy decides)

- **Phase 6, in-app audio engine** (ten-band EQ applied to system audio via a macOS Audio Server
  plug-in): needs Roy's Apple Developer account and a one-time admin install. Research:
  `docs/research/reports/Advanced equalizer research.html`. Undecided.
- **Phase 9, other Sony v2 models** (XM5 etc.): NC/EQ decoders per model, name-based discovery
  on Windows/Linux. Proposed, undecided.
- **Phase 7b / 8b**: Windows audio "playing" and microphone sensors are blank; Linux never run on
  hardware (`SHDC_RFCOMM_CHANNEL=18` skips the channel scan on his unit).
- **Signing**: macOS is ad-hoc only (Gatekeeper: System Settings → Privacy & Security → Open Anyway,
  or `xattr -dr com.apple.quarantine`); Windows unsigned (SmartScreen). Workflow already takes
  Apple secrets if Roy joins the Developer Program.
- **High-contrast accessibility theme** (thicker rules, larger type), deliberately not done.
- **Advanced equalizer strip colour**: pink vs orange; orange today.
- **Design file** still says "Turn DSEE off at low battery"; the app says "DSEE Extreme".
- **Hardware validation checklists** `docs/hardware-validation.md` §6–§11 (Scenes, Phase 4,
  equalizer, Windows app, Linux) are mostly unticked.
- **Housekeeping**: remote branches `phase1-core-and-popover`, `phase2-main-window`,
  `phase3-windows-transport`, `phase7-windows-app`, `phase8-linux` are merged and can be deleted
  (branch deletion was blocked for Claude); back up `~/.tauri/hairline.key` (losing it breaks
  future updates for every installed copy); downgrading below 0.4.0 after picking a new theme resets
  that older build's settings file.

## 7. How to resume

1. Read this file, `CLAUDE.md`, and the latest `.github/release-notes.md`.
2. `git pull`; `cargo test --workspace`; `cd apps/desktop && pnpm install && pnpm check && pnpm test`.
3. Run the app: `cd apps/desktop && SHDC_FAKE=1 pnpm tauri dev` (fake headphones) or without the
   variable with the real unit paired.
4. For a new feature: branch from main, implement with tests, get at least one independent review
   (a subagent reading the diff), fix, merge with `--no-ff`, bump version, notes, tag, watch the run.
5. Ask Roy for the Windows results in §5 before touching the Windows code again.
