# Hairline

Hairline is a desktop client for Sony ULT WEAR (WH-ULT900N) headphones. It talks to the headphones directly over Bluetooth, without the official phone app, and puts the everyday controls in a small menu-bar popover.

## What it looks like

The Hairline design: off-white paper, one-pixel rules, dot-matrix numerals, boxed switches, and orange only where something is live. The popover is what you use every day from the menu bar; the window is where you set everything else up.

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

Phase 2: the full window, opened from the popover footer (it loads the app with `?view=main`). It has five screens: Overview, Sound (a draggable six-band equalizer, presets, ULT, codec and DSEE), Noise, Devices and Settings. It also adds Paper, Ink and System themes and launch at login. By default Hairline shows a Dock icon only while the window is open (Settings can keep it there).

Phase 2b: Modes (Scenes), described below. It is verified against the fake device and still needs a session with the real headphones.

Phase 3a: Windows transport compiles and is unit-tested on Windows CI; the Windows tray app and hardware validation are next. It has not yet been run against the headphones on Windows.

Phase 4: the Advanced panel, the opcode probe, battery history with a time-left estimate, and a local socket with the `hairlinectl` command-line tool, all described below. It is verified against the fake device and the unit tests; the checklist for the real headphones is section 8 of `docs/hardware-validation.md`.

Next: the advanced equalizer (Phase 5), then the Windows tray app, then Linux. The pipeline is in [the equalizer research](<docs/research/reports/Advanced equalizer research.html>).

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

Hotkeys are recorded in the scene editor as Tauri accelerators and registered with the global-shortcut plugin only while Hairline runs. If one conflicts or the OS refuses it, it shows as refused, with the reason, in the editor and in Settings. A scene hotkey needs Ctrl, Alt or Super (or is a lone F1–F24 key), so it cannot take over typing. `⌥⌘N` opens the window when macOS allows the shortcut; Settings shows it refused otherwise.

## Advanced panel

Settings has an "Advanced panel" switch; turning it on adds the Advanced screen. It is off by default and has three parts:

- Capabilities: every function the headphones declare, with its name. A chip says DECLARED (the headphones listed it) or PROVEN (it answered the probe although it was not declared, for this unit).
- Frame log: the frames going to and from the headphones, live. Record writes them to a file in `~/Library/Logs/com.drake0306.hairline/captures/` (macOS), and Stop ends the file.
- Raw command: type the bytes in hex and pick the table. A GET is sent directly. Any other opcode needs you to type `SEND` first. The deny-list is absolute: a denied payload is refused even with `SEND`, and there is no reset anywhere in the app. Raw commands need the headphones connected. The note under Send says "Queued; reply shows in the frame log" once the request is queued, or why it was refused.

## Probe

The probe, on the Advanced screen, walks the read-only requests of the protocol to find functions the headphones answer but do not declare. It sends 46 GET opcodes, 441 requests in all, one at a time, waiting 1.5 s for a reply and 150 ms between requests. That is about 13 minutes. You start it by typing `RUN`, and Stop ends it at any time. It never sends a SET.

Results go to `~/Library/Logs/com.drake0306.hairline/captures/probe-<secs>.json`. A function that answered although the headphones did not declare it becomes "proven" for this unit and is remembered in `devices/<address>.json`. Hairline may then read it, but never changes it. Two reads are left out: reset settings, and the LE Audio identity key (its reply is a pairing secret). The catalogue is in `crates/mdr-proto/src/probe.rs`.

## Battery history

While the headphones are connected, Hairline records the battery level to `battery.jsonl`, next to `settings.json`. The time left comes from a least-squares fit over the current discharge segment. It shows only after at least 30 minutes of discharge and a drop of at least 2 %. A gap of more than 2 hours, or charging, starts a new segment. The estimate is refreshed every 60 seconds. Clear in Settings deletes the history.

## Command line

While Hairline runs, it listens on a local socket, `hairline.sock`, in the app's data directory (`~/Library/Application Support/com.drake0306.hairline/` on macOS). The protocol is JSON lines, and the commands are state, set, scene, scenes and battery. The socket has no raw, power or probe command. `hairlinectl` (crate `crates/mdr-cli`) talks to it:

```
cargo run -p mdr-cli --bin hairlinectl -- state
hairlinectl nc ambient --level 12 --voice on
hairlinectl ult 2
hairlinectl volume 20
hairlinectl eq "bass boost"
hairlinectl scenes
hairlinectl scene "At the desk"
hairlinectl battery
hairlinectl --json state
```

`state`, `nc`, `ult`, `volume`, `eq`, `scene`, `scenes` and `battery` are the subcommands. `--json` prints the app's raw answer, and `$HAIRLINE_SOCKET` points it at another socket. It exits with 1 when the app is not running or refuses the request. On Windows it prints that it is not supported yet.

## Run it

Prerequisites: Rust stable, Node 22, pnpm 10.

```
cd apps/desktop
pnpm install
pnpm tauri dev -- -- --fake-device
```

The double `--` is intentional: the Tauri CLI forwards the first one to cargo, and the second hands `--fake-device` to the app. Setting the environment variable also works:

```
HAIRLINE_FAKE=1 pnpm tauri dev
```

`--fake-device` runs the app against a simulated headphone instead of a real Bluetooth connection, so you can try the popover without the hardware.

## Tests

```
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cd apps/desktop && pnpm check && pnpm test
```

## Debugging

`HAIRLINE_FRAME_LOG=1` writes every frame and session log line to `<YYYY-MM-DD-HHMMSS>.log` (time in UTC) in the capture directory: `~/Library/Logs/com.drake0306.hairline/captures/` on macOS, `captures` in the app's `logs` directory under the local data directory on Windows and Linux (for example `%LOCALAPPDATA%\com.drake0306.hairline\logs\captures\`). The probe results and the Record button write there too.
To record from a running app instead, turn on the Advanced panel and press Record in the frame log. Raw commands are sent from the same screen; the deny-list still applies.

## Layout

- `crates/mdr-frame`: packet framing, escaping and checksums.
- `crates/mdr-proto`: message encoding and decoding, and the request deny-list.
- `crates/mdr-session`: handshake, acknowledgements and retries, and device state.
- `crates/mdr-transport`: the Bluetooth link to the headphones.
- `apps/desktop`: the Tauri 2 and Svelte 5 app (Rust side in `src-tauri`).

## Safety

Hairline never sends commands on the deny-list. The app, the scenes and the socket change a setting only when the headphones declare its function; a function the probe proved (it answered although it was not declared) unlocks reads, never changes. The only way to send a SET for anything else is a raw command in the Advanced panel, which asks you to type `SEND` first and cannot get past the deny-list. The probe sends only GETs, and neither the probe nor the socket can send a raw command. The one exception to the deny-list is power off, after the user confirms it. The details are in [docs/protocol-notes.md](docs/protocol-notes.md).

## Design

The approved interface is in [docs/research/reports/Hairline design screens.html](<docs/research/reports/Hairline design screens.html>). The advanced equalizer plan is in [the research](<docs/research/reports/Advanced equalizer research.html>). The full design is in [the spec](docs/superpowers/specs/2026-11-07-ult-wear-desktop-app-design.md).

## License

MIT. Third-party notices are in [THIRD_PARTY.md](THIRD_PARTY.md).

Not affiliated with or endorsed by Sony. Sony, ULT WEAR, DSEE and LDAC are trademarks of their owners.
