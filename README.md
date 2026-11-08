# SHDC

**Sony Headphone Desktop Client.**

SHDC is a desktop client for Sony ULT WEAR (WH-ULT900N) headphones. It talks to the headphones directly over Bluetooth, without the official phone app, and puts the everyday controls in a small menu-bar popover.

## What it looks like

The interface keeps its design name, Hairline: off-white paper, one-pixel rules, dot-matrix numerals, boxed switches, and orange only where something is live. The popover is what you use every day from the menu bar; the window is where you set everything else up.

<table>
  <tr>
    <td align="center" width="50%"><img src="docs/screenshots/01-menu-bar-popover.png" alt="Menu-bar popover, connected and not connected" width="100%"><br><sub>Menu-bar popover</sub></td>
    <td align="center" width="50%"><img src="docs/screenshots/02-overview.png" alt="Overview screen" width="100%"><br><sub>Overview</sub></td>
  </tr>
  <tr>
    <td align="center"><img src="docs/screenshots/03-sound.png" alt="Sound screen with the fader equalizer" width="100%"><br><sub>Sound</sub></td>
    <td align="center"><img src="docs/screenshots/04-noise.png" alt="Noise screen" width="100%"><br><sub>Noise</sub></td>
  </tr>
  <tr>
    <td align="center"><img src="docs/screenshots/05-devices.png" alt="Devices screen" width="100%"><br><sub>Devices</sub></td>
    <td align="center"><img src="docs/screenshots/06-scenes.png" alt="Scenes screen" width="100%"><br><sub>Scenes</sub></td>
  </tr>
  <tr>
    <td align="center" colspan="2"><img src="docs/screenshots/07-settings.png" alt="Settings screen" width="50%"><br><sub>Settings</sub></td>
  </tr>
</table>

The popover shipped in Phase 1 and is validated on real hardware; the window screens shipped in Phase 2, built from these mockups (`docs/research/reports/Hairline design screens.html`). The Scenes screen shipped in Phase 2b (the modes module, below); it is verified against the fake device only.

## Status

Phase 1: a macOS menu-bar popover for the WH-ULT900N with battery level, noise control, ambient sound level and ULT level.

Phase 2: the full window, opened from the popover footer (it loads the app with `?view=main`). It has five screens: Overview, Sound (a draggable six-band equalizer, presets, ULT, codec and DSEE), Noise, Devices and Settings. It also adds Paper, Ink and System themes and launch at login. By default SHDC shows a Dock icon only while the window is open (Settings can keep it there).

Phase 2b: Modes (Scenes), described below. It is verified against the fake device and still needs a session with the real headphones.

Phase 3a: Windows transport compiles and is unit-tested on Windows CI; the Windows tray app and hardware validation are next. It has not yet been run against the headphones on Windows.

Phase 4: the Advanced panel, the opcode probe, battery history with a time-left estimate, and a local socket with the `shdcctl` command-line tool, all described below. It is verified against the fake device and the unit tests; the checklist for the real headphones is section 8 of `docs/hardware-validation.md`.

Phase 5: the Advanced equalizer, described below, and a redesigned Sound fader (21 segments with a zero line, an orange knob and a dot-matrix value). It is verified against the fake device and the unit tests; the checklist for the real headphones is section 9 of `docs/hardware-validation.md`.

Phase 7a: the app builds and its tests pass on Windows CI. The tray popover is a plain window there, placed just above the taskbar and centred on the tray icon from the monitor's work area (0.3.2; the first Windows run put it far up the screen); audio 'playing' and the microphone trigger are blank until Windows modules exist. Not run against the headphones on Windows. The checklist for a Windows machine is section 10 of `docs/hardware-validation.md`.

Phase 8a: the Linux transport and the app build and pass tests on Linux CI; not run against the headphones on Linux. The tray needs libayatana-appindicator; audio 'playing' and the microphone trigger are blank until Linux modules exist. The checklist for a Linux machine is section 11 of `docs/hardware-validation.md`.

Next: running the Windows and Linux apps on real machines. The in-app audio engine (Phase 6) is undecided; see [the equalizer research](<docs/research/reports/Advanced equalizer research.html>).

The popover is validated on a real WH-ULT900N (firmware 2.1.4) on macOS 27.2; see `docs/hardware-validation.md`. The Phase 2 screens are verified against the fake device and still need a session with the real headphones. Use `--fake-device` (below) to run without headphones; a fake run says `FAKE` in the popover and the window so it is never mistaken for the real thing.

## Modes (Scenes)

A scene is a named set of settings plus a trigger. Scenes live in `scenes.json`, next to `settings.json`. Four ship disabled, the more specific triggers first: "On a call", "Late", "At the desk" and "Walking". The Scenes screen edits them; the popover MODE row shows up to four enabled scenes, and a "…" opens the Scenes screen when there are more.

Triggers on macOS:

- By hand: only when you apply the scene.
- Headphones connect.
- Time range: from and to, and it can wrap midnight.
- App in front: the frontmost app's name, compared case-insensitively.
- Microphone in use: the default input device is running (a call, a recording).

Every 2 seconds the broker thread checks the enabled scenes in list order, and the first one that matches wins. List order is the priority; the Up and Down buttons move a scene. A scene applied by hand stays active until another one matches. When a trigger stops matching, nothing is reverted. If the headphones are not connected, the active scene is tracked and applied on connect, after a fresh check.

Actions use the existing controls only: noise control, ambient level and voice focus (sent as one request), ULT and an EQ preset (also one request), volume, and DSEE auto or off. "Keep" leaves a setting as it is. A setting the headphones refuse shows on the normal controls like any other refusal. Scenes send nothing new on the wire.

Hotkeys are recorded in the scene editor as Tauri accelerators and registered with the global-shortcut plugin only while SHDC runs. If one conflicts or the OS refuses it, it shows as refused, with the reason, in the editor and in Settings. A scene hotkey needs Ctrl, Alt or Super (or is a lone F1–F24 key), so it cannot take over typing. `⌥⌘N` opens the window when macOS allows the shortcut; Settings shows it refused otherwise.

## Sound

The Sound screen shows the headphones' six EQ bands as faders, the preset grid, ULT, the codec and DSEE, all as the headphones report them. The faders can be moved only on Manual, Custom 1 and Custom 2. On Sony's fixed presets (Bright, Excited, Mellow, Relaxed, Vocal, Treble Boost, Bass Boost, Speech) the headphones apply the curve inside and report every band at 0, so the faders are read-only and say so: "Sony applies this preset inside the headphones and does not report its band values. Choose Manual, Custom 1 or Custom 2 to shape your own." On Off the faders are disabled ("Equalizer off."). SHDC never makes up band values for a fixed preset.

## Advanced equalizer

The headphones have six EQ bands. The Advanced equalizer is a ten-band curve (31 Hz to 16 kHz) that SHDC keeps itself. Open it with the "Advanced" button next to the Equalizer heading on the Sound screen; it takes the whole content area and "‹ Sound" returns. It shows four of the ten bands at a time, and ‹ and › page through them (1–4, 5–8, 7–10).

Every change is saved to `eq_curve.json`, next to `settings.json` in the app's config directory. Nothing is sent to the headphones while you edit. These bands run in SHDC only; the headphones keep their six-band curve.

"Send best fit to headphones" fits the ten bands onto the six: Clear Bass is the mean of the four lowest bands, 1 kHz and 16 kHz are taken directly, and 400 Hz, 2.5 kHz and 6.3 kHz are interpolated in log frequency between their neighbours (rounded half away from zero, clamped to −10..+10). The view shows the headphones' current six values beside the fit.

Send asks first. A dialog, "Send best fit to the headphones?", says that the headphones hold six bands (Clear Bass, 400 Hz, 1 kHz, 2.5 kHz, 6.3 kHz, 16 kHz), not ten, that SHDC fits the ten onto those six, which preset they go into, and lists the six fitted values. Nothing is sent until you press Send in the dialog; Cancel, Escape or a click outside it sends nothing.

Which preset: if the headphones are on Manual, Custom 1 or Custom 2, the fit is written into that preset. On any other preset (a fixed Sony preset or Off) it is written into Custom 1, and the dialog says "Custom 1 (the headphones will switch to it)". The write is one normal EQ command (`58 03`) that carries the target preset and the six bands together and keeps ULT as it is; no new command goes on the wire.

Reset, beside Send, puts all ten bands back to 0 and saves that curve. It does not touch the headphones.

## Sharing a preset

An equalizer setting can be passed to someone else as a 15-character code, shown as `XXXXX-XXXXX-XXXXX`, with a QR of the same text.

**Share.** On the Sound screen, "Share" (in the Presets block) is available on Manual, Custom 1 and Custom 2 while the headphones are connected; it encodes the six bands the headphones report, the ULT level and which of the three presets it came from. It is unavailable on a fixed Sony preset or Off (Sony does not report a fixed preset's bands), with no headphones, while an EQ change is still pending, and on headphones whose equalizer is not six bands of 21 steps. In the Advanced equalizer, "Share" and "Paste a code" sit at the right of the top row; "Share" encodes the ten-band curve. Either opens "Share this preset" / "Share this curve" with the QR, the code and Copy.

**Paste.** "Paste a code" (on the Sound screen and in the Advanced equalizer) opens a field. The code is checked as you type, with or without dashes (any kind of dash), in any case; the values it carries are shown under it, or "That code is not an SHDC preset." / "That code has a typo (check failed).". Apply then:

- a Sound code goes to the headphones only after the dialog "Apply this preset to the headphones?", which names the target preset and the ULT level and lists the six values. The target is the preset the code came from (Manual, Custom 1 or Custom 2); a code without one follows the Send best fit rule (the current preset if it is Manual, Custom 1 or Custom 2, otherwise Custom 1). The write is the same single `58 03 <preset> <ULT> 06 <six bands>` frame the app already uses; Cancel or Escape sends nothing.
- an Advanced code is saved as the ten-band curve (drawn once the save succeeds) and opens the Advanced equalizer. Nothing is sent to the headphones.

**The code.** The alphabet is Crockford base32, `0123456789ABCDEFGHJKMNPQRSTVWXYZ` (no I, L, O or U). When reading, whitespace and any dash (hyphen, en or em dash) are dropped, letters are upper-cased, I and L read as 1 and O as 0. Each symbol is 5 bits, 75 bits in all, most significant bit first:

| Symbols | Bits | Content |
| --- | --- | --- |
| 1 | 5 | Kind, as the symbol's value: `S` (25) Sound, `A` (10) Advanced. Any other kind is rejected. |
| 2–13 | 60 | Sound: six raw bands 0..20 (10 is flat; Clear Bass, 400 Hz, 1 kHz, 2.5 kHz, 6.3 kHz, 16 kHz), 5 bits each; ULT 0..2 in 2 bits; slot in 2 bits (0 Manual, 1 Custom 1, 2 Custom 2, 3 none); then 26 zero bits. Advanced: ten bands −10..+10 stored as level + 10 (31 Hz .. 16 kHz), 5 bits each; then 10 zero bits. |
| 14–15 | 10 | CRC-10 over the 65 bits of symbols 1–13: polynomial 0x233, initial value 0, no reflection, no final xor. |

The dashes go after symbols 5 and 10. A band above 20, ULT 3, or padding that is not zero is "not an SHDC preset". A ten-bit CRC catches every error within ten consecutive bits, so any single wrong symbol and any swapped neighbouring pair fail the check. For example, Custom 1 with bands 15 11 8 11 13 16 and ULT 1 is `SFB8B-DGA00-000W7`, and the curve 8 6 4 2 0 −2 0 2 4 −10 is `AJGEC-A8ACE-000ND`. The codec is `apps/desktop/src/lib/presetCode.ts`.

The QR holds just the code text (no URL), at error-correction level M, drawn with qrcode-generator (MIT, see `THIRD_PARTY.md`).

## Advanced panel

Settings has an "Advanced panel" switch; turning it on adds the Advanced screen. It is off by default and has three parts:

- Capabilities: every function the headphones declare, with its name. A chip says DECLARED (the headphones listed it) or PROVEN (it answered the probe although it was not declared, for this unit).
- Frame log: the frames going to and from the headphones, live. Record writes them to a file in `~/Library/Logs/com.drake0306.hairline/captures/` (macOS), and Stop ends the file.
- Raw command: type the bytes in hex and pick the table. A GET is sent directly. Any other opcode needs you to type `SEND` first. The deny-list is absolute: a denied payload is refused even with `SEND`, and there is no reset anywhere in the app. Raw commands need the headphones connected. The note under Send says "Queued; reply shows in the frame log" once the request is queued, or why it was refused.

## Probe

The probe, on the Advanced screen, walks the read-only requests of the protocol to find functions the headphones answer but do not declare. It sends 46 GET opcodes, 441 requests in all, one at a time, waiting 1.5 s for a reply and 150 ms between requests. That is up to about 13 minutes (less when requests are answered). You start it by typing `RUN`, and Stop ends it at any time. It never sends a SET.

Results go to `~/Library/Logs/com.drake0306.hairline/captures/probe-<secs>.json`. A function that answered although the headphones did not declare it becomes "proven" for this unit and is remembered in `devices/<address>.json`. SHDC may then read it, but never changes it. Two reads are left out: reset settings, and the LE Audio identity key (its reply is a pairing secret). The catalogue is in `crates/mdr-proto/src/probe.rs`.

## Battery history

While the headphones are connected, SHDC records the battery level to `battery.jsonl`, next to `settings.json`. The time left comes from a least-squares fit over the current discharge segment. It shows only after at least 30 minutes of discharge and a drop of at least 2 %. A gap of more than 2 hours, or charging, starts a new segment. The estimate is refreshed every 60 seconds. Clear in Settings deletes the history.

## Command line

While SHDC runs, it listens on a local socket, `shdc.sock`, in the app's config directory, next to `settings.json` (on macOS that is the same as the data directory, `~/Library/Application Support/com.drake0306.hairline/`). The protocol is JSON lines, and the commands are state, set, scene, scenes and battery. The socket has no raw, power or probe command. `shdcctl` (crate `crates/mdr-cli`) talks to it. Install it once, then use it from any terminal:

```
cargo install --path crates/mdr-cli
shdcctl state
shdcctl nc ambient --level 12 --voice on
shdcctl ult 2
shdcctl volume 20
shdcctl eq "bass boost"
shdcctl scenes
shdcctl scene "At the desk"
shdcctl battery
shdcctl --json state
```

`state`, `nc`, `ult`, `volume`, `eq`, `scene`, `scenes` and `battery` are the subcommands. `--json` prints the app's raw answer, and `$SHDC_SOCKET` points it at another socket. It exits with 1 when the app is not running or refuses the request. On Windows it prints that it is not supported yet.

## Install

Builds for macOS and Windows are on the [Releases page](https://github.com/Drake0306/Sony-Headphone-PC_Client/releases).

macOS: open the `.dmg` and drag SHDC to Applications. Open it once; macOS says it could not verify the app because it is not signed with an Apple Developer certificate yet. Click Done, then go to System Settings → Privacy & Security, scroll to Security, click Open Anyway, enter your password and click Open. After that it opens normally. Terminal alternative: `xattr -dr com.apple.quarantine /Applications/SHDC.app`. Pair the headphones in System Settings → Bluetooth first.

Windows: run the `.msi` or the `-setup.exe`. If SmartScreen appears, choose More info → Run anyway. The Windows build compiles and passes its tests but has not been tried with real headphones on Windows.

## Updates

SHDC checks the latest GitHub release for a newer version 15 seconds after it starts and every 6 hours while it runs. It reads `latest.json` from the release; nothing else is sent. A failed check (offline, GitHub down) shows nothing.

When a newer version is out, Settings shows "SHDC 0.x.y is available" with the release notes, and the popover shows "Update available · open SHDC". Later hides that version until SHDC is next started. Update now downloads it with a progress bar, installs it and relaunches SHDC as the new version. On Windows the installer takes over and starts SHDC again when it is done; it may ask for permission. Nothing is downloaded or installed unless you click Update now. Check now in Settings asks at any time.

The feed lives in two places, and the app tries them in order: `latest.json` on the latest GitHub release, then the same file on the repository's `updates` branch, served by raw.githubusercontent.com. The download links inside point at api.github.com, which redirects to the file. This matters because github.com's release download host can fail at one GitHub edge for a while (it returned 504 from India on 8 November 2026) while the API and the file store still answer. The release workflow's `feed` job writes both copies with `.github/scripts/api-feed.py`.

`SHDC_UPDATE_FEED=https://…/latest.json` makes the app check that one feed instead, for testing a staged release. Only https is accepted.

Every update is signed in the release workflow, and SHDC carries the public key. A download whose signature does not match is not installed, and Settings says "Update could not be verified".

To stop the automatic checks, switch off "Check for updates automatically" in Settings. Check now still works. Switching it back on takes effect the next time SHDC starts.

## Run it

Prerequisites: Rust stable, Node 22, pnpm 10.

```
cd apps/desktop
pnpm install
pnpm tauri dev -- -- --fake-device
```

The double `--` is intentional: the Tauri CLI forwards the first one to cargo, and the second hands `--fake-device` to the app. Setting the environment variable also works:

```
SHDC_FAKE=1 pnpm tauri dev
```

`--fake-device` runs the app against a simulated headphone instead of a real Bluetooth connection, so you can try the popover without the hardware.

Linux: install the build libraries first (Debian and Ubuntu names):

```
sudo apt-get install libdbus-1-dev pkg-config libwebkit2gtk-4.1-dev \
  libayatana-appindicator3-dev librsvg2-dev patchelf libgtk-3-dev libxdo-dev libssl-dev
```

The tray icon needs a desktop with StatusNotifierItem support (on GNOME, the AppIndicator extension). Linux trays report no clicks, so the popover opens from the tray menu's "Show SHDC". BlueZ does not expose SDP over D-Bus, so the transport finds the MDR v2 RFCOMM channel itself: `SHDC_RFCOMM_CHANNEL=18` names it; otherwise it uses the channel recorded at the last connect. Either is used as soon as it accepts the connection, and the session's handshake proves it. If neither connects, it scans the other channels 1 to 30, sending each only the protocol-info GET (`00 00`) and keeping the first that answers as v2. A channel that refuses, is busy (held by HFP or another program) or does not connect within 5 seconds is skipped. The channel used is printed (`hairline: RFCOMM channel 18 (channel scan)`) and recorded in `devices/<address>.json`.

## Tests

```
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cd apps/desktop && pnpm check && pnpm test
```

## Debugging

`SHDC_FRAME_LOG=1` writes every frame and session log line to `<YYYY-MM-DD-HHMMSS>.log` (time in UTC) in the capture directory: `~/Library/Logs/com.drake0306.hairline/captures/` on macOS, `captures` in the app's `logs` directory under the local data directory on Windows and Linux (for example `%LOCALAPPDATA%\com.drake0306.hairline\logs\captures\`). The probe results and the Record button write there too.
To record from a running app instead, turn on the Advanced panel and press Record in the frame log. Raw commands are sent from the same screen; the deny-list still applies.

## Layout

- `crates/mdr-frame`: packet framing, escaping and checksums.
- `crates/mdr-proto`: message encoding and decoding, and the request deny-list.
- `crates/mdr-session`: handshake, acknowledgements and retries, and device state.
- `crates/mdr-transport`: the Bluetooth link to the headphones.
- `apps/desktop`: the Tauri 2 and Svelte 5 app (Rust side in `src-tauri`).

## Safety

SHDC never sends commands on the deny-list. The app, the scenes and the socket change a setting only when the headphones declare its function; a function the probe proved (it answered although it was not declared) unlocks reads, never changes. The only way to send a SET for anything else is a raw command in the Advanced panel, which asks you to type `SEND` first and cannot get past the deny-list. The probe sends only GETs, and neither the probe nor the socket can send a raw command. The one exception to the deny-list is power off, after the user confirms it. The details are in [docs/protocol-notes.md](docs/protocol-notes.md).

## Design

The approved interface is in [docs/research/reports/Hairline design screens.html](<docs/research/reports/Hairline design screens.html>). The advanced equalizer plan is in [the research](<docs/research/reports/Advanced equalizer research.html>). The full design is in [the spec](docs/superpowers/specs/2026-11-07-ult-wear-desktop-app-design.md).

## License

MIT. Third-party notices are in [THIRD_PARTY.md](THIRD_PARTY.md).

Not affiliated with or endorsed by Sony. Sony, ULT WEAR, DSEE and LDAC are trademarks of their owners.
