# Hardware validation — WH-ULT900N

Date: 2026-11-07. Host: MacBook, macOS 27.2 (build 26B123), Apple Silicon. Headphones: Sony ULT WEAR, model WH-ULT900N, Bluetooth address `a8-e6-e8-fe-d6-54`, firmware **2.1.4**, colourway Black (device reports series/colour `11 01`). Phone Bluetooth off during all tests. Tools: the `mdr-*` crates at commit 3598ee6 driven by a small read-only probe binary (not in the repo).

## 1. Connection

| Check | Result |
|---|---|
| Paired device visible to IOBluetooth | Yes, name `ULT WEAR`, address in lowercase-dash form |
| Cached SDP records while disconnected | None (0 records); `performSDPQuery` yields nothing while disconnected |
| Cached SDP records while connected | 22 records; v2 UUID `956C7B26-…` → "SPP Serial Port", RFCOMM channel **18** |
| Other channels seen | 15 "SPP Serial Port" (opens, accepts writes, never answers MDR frames), 22 unnamed (opens and immediately pushes `FE 03 01 00 00 F3 00 F3 …`, left alone), 30 `Airoha_IAP2`, 21 `Airoha_APP` (never touched), 19/20 Google, 23 Fast Pair, 8 `BTNOTIFYR`, 13 `BT_SAR_AP`, 12/14 Amazon Music/Spotify, HFP 1, Headset 2 |
| `openRFCOMMChannelSync` inside a main-queue dispatch block | **Fails** immediately with `kIOReturnError` (0xe00002bc) on a fresh link, every time, and leaves DLCI 18 half-open: later opens hang (no UA, no DM) and did not recover by themselves within about 105 s |
| `openRFCOMMChannelSync` directly on the main thread | Succeeds (about 2.7 s) |
| `openRFCOMMChannelAsync` + run-loop completion (shipped fix) | Succeeds (about 1.3 s) |
| Clearing a stuck DLCI | `IOBluetoothDevice.closeConnection()` clears it; macOS did not re-establish the audio link by itself within a few seconds afterwards, `openConnection()` was needed |

## 2. Handshake (all read-only)

| Request | Reply (payload) | Meaning |
|---|---|---|
| T1 `00 00` | `01 00 03 00 20 02 00 00` | v2, protocol 0x03002002, tables 1 and 2 |
| T1 `06 00` | 30 functions: `10 12 13 14 15 20 23 25 27 32 53 69 6B 70 93 F4 90 A1 F1 B2 B7 C1 C2 D1 D2 D3 E3 E2 F9 FD` (with priorities) | Identical to the list in the research notes for fw 2.1.0 |
| T2 `06 00` | `42 50 F2` | Voice guidance (MTK), Safe Listening HBS1, Quick Access easy setting |
| T1 `04 01` | `ULT WEAR` | Model name |
| T1 `04 02` | `2.1.4` | Firmware |
| T1 `04 03` | `11 01` | Series 0x11, colour 0x01 = Black |
| T1 `22 00` | `23 00 33 00` | Battery 51 %, not charging |
| T1 `66 17` | `67 17 01 01 01 00 14` | Ambient mode, voice focus off, level 20 |
| T1 `50 03` | `51 03 06 15 0A 0C` + 12 ids with **empty names** | 6 bands, 21 steps, ULT2 extra step 10; preset ids `00 10 11 12 13 14 15 16 17 A0 A1 A2`. The device sends no preset names; the UI must map ids itself |
| T1 `56 03` | `57 03 A2 01 06 0A 0D 0A 0E 14 0C` | Preset 0xA2 (custom slot 2), ULT 1, bands Clear Bass 0, +3, 0, +4, +10, +2 (centre 0x0A) |
| T1 `12 02` | `13 02 02` | Codec AAC (macOS link) |
| T1 `D0 D1` / `D6 D1` | boolean, `MULTIPOINT_SETTING` / `MULTIPOINT_SETTING_SUMMARY`; value `01` | Multipoint off (0x00 = on, 0x01 = off) |
| T1 `D0 D2` / `D6 D2` | boolean, `TOUCH_PANEL_SETTING`; value `01` | Touch panel setting value 0x01 (**meaning to confirm**: 0x00 = on per the general-setting enum, so this reads as off) |
| T1 `D0 D3` / `D6 D3` | boolean, `SIDETONE_SETTING` / `…_SUMMARY`; value `00` | Sidetone on |
| T1 `26 05` | `27 05 10 11` | Auto power-off: current "when removed from ears" (0x10), last selected "disabled" (0x11) |
| T1 `A6 20` | `A7 20 1C` | Volume 28 of 30 |

Frame-level: every device data frame carried the expected sequence alternation and was ACKed; the device ACKed every request within the 10 ms receive slice. No retransmits occurred.

## 3. Undeclared commands that answer (read-only GETs)

| Request | Reply | Notes |
|---|---|---|
| T2 `32 02` (peripheral status) | `33 02 00 01` | Answers although no peripheral function is declared. Candidate for the per-unit "proven" list (GET only). |
| T2 `36 02` (remembered devices) | silent | Not supported on this model, as the research expected |
| T2 `46 20` (voice guide volume) | `47 20 02` | Volume +2 (range −2..2) |
| T1 `A2 01` (playback status) | `A3 01 00 02 00` | Playback status with call-volume adjustment type; byte meaning to map against libmdr `PlayRetStatus` |
| T1 `12 03` (DSEE indicator) | `13 03 01 01` | Effect type DSEE, status on |
| T1 `E6 02` (connection mode with LDAC status) | `E7 02 00` | Value 0x00 (sound-quality priority per libmdr enum; **confirm** against a deliberate switch later) |
| T1 `F6 01` (pause when taken off) | `F7 01 01` | Value 0x01 (**meaning to confirm**: libmdr `OnOffSettingValue`) |
| T1 `F6 04` (voice assistant) | `F7 04 FF` | No function assigned |
| T1 `F6 0D` (Quick Access) | `F7 0D 02 00 00` | Two slots, nothing assigned |

## 4. Controls from the popover (owner-driven, app build at commit 3598ee6)

Clicked by the owner in the running app; frames from `captures/local/2026-11-06-213050.log`. Every SET was ACKed and followed by the matching NTFY within about 250–350 ms, and the popover showed the striped pending state and then the confirmed state.

| Action | SET sent | NTFY received | Result |
|---|---|---|---|
| Noise cancelling, level 1 | `68 17 01 01 00 00 01` | `69 17 01 01 00 00 01` | Confirmed |
| Off | `68 17 01 00 00 00 01` | `69 17 01 00 01 00 01` | Confirmed (device reports mode byte 0x01 while total effect is off) |
| Noise cancelling again | `68 17 01 01 00 00 01` | `69 17 01 01 00 00 01` | Confirmed |
| Ambient, level 20 | `68 17 01 01 01 00 14` | `69 17 01 01 01 00 14` | Confirmed |
| Noise cancelling, level 20 kept | `68 17 01 01 00 00 14` | `69 17 01 01 00 00 14` | Confirmed |
| ULT 2 | `58 03 A2 02 06 0A 0D 0A 0E 14 0C` | `59 03 A2 02 06 0A 0D 0A 0E 14 0C` | Confirmed; preset and bands preserved (read-modify-write works) |
| ULT 1 | `58 03 A2 01 06 0A 0D 0A 0E 14 0C` | `59 03 A2 01 06 0A 0D 0A 0E 14 0C` | Confirmed |
| Ambient, level 20, voice focus on | `68 17 01 01 01 01 14` | `69 17 01 01 01 01 14` | Confirmed |
| Voice focus off | `68 17 01 01 01 00 14` | `69 17 01 01 01 00 14` | Confirmed |

Menu bar: the owner reports the tray icon and popover look right ("looks good").

## 5. Not yet done

- EQ-type experiment (`58 03` vs `58 00` for changing the preset); waits for the owner's go-ahead. Current preset 0xA2 with bands `0A 0D 0A 0E 14 0C` is recorded above for restoration.
- Phone-contention test (requires the phone).
- Hide-on-focus-loss and popover placement in dark mode: to be confirmed by the owner.

## 6. Scenes (Phase 2b, to do on the owner's next session)

Run with `HAIRLINE_FRAME_LOG=1` and check each result in the log under `~/Library/Logs/com.drake0306.hairline/captures/` (the app's log directory; on Windows and Linux, `captures` in the app's local data `logs` directory).

- [ ] Apply a scene by hand from the Scenes screen, then another from the popover MODE row. The controls update and the headphones confirm.
- [ ] HeadphonesConnected: enable "At the desk", reconnect the headphones. Expect Ambient 14 with voice focus on, and nothing else changed.
- [ ] MicInUse: enable "On a call", start a FaceTime or Zoom test call. Expect noise cancelling. In the frame log, confirm the scene is not re-sent every 2 s.
- [ ] TimeRange: set a range that includes now and one that does not; the scene applies only inside it.
- [ ] FrontmostApp: use a real app name (try different capitalisation); the scene applies when that app is in front.
- [ ] Hotkey: record one on a scene and press it. Press `⌥⌘N` and confirm the window opens.
- [ ] Hotkey conflict: record one that another app already uses and record what macOS does when another app holds the same combination (refused with the reason in the editor and in Settings, or taken silently).
- [ ] Keep: a scene with a Keep field leaves that setting untouched: no SET changes it (the value inside a combined NC or EQ request is unchanged).

## 7. Windows (Phase 3a, to do when a Windows machine is available)

The Windows transport compiles and passes unit tests on Windows CI; it has not yet been run against the headphones on Windows. Nothing below is verified.

- [ ] Discovery: with the headphones paired and connected, `discover()` lists them with `connected_audio: true`; paired but disconnected, they are listed with `connected_audio: false`.
- [ ] Connect while disconnected fails at once with `NotConnected` and does not hang.
- [ ] Connect while connected opens the v2 channel (stack resolves it from SDP); record the channel `getpeername` reports, or that it reports none.
- [ ] Handshake completes read-only (the section 2 sequence) over the Winsock link.
- [ ] `recv_timeout` returns `None` on a quiet link and an error when the headphones are switched off.
- [ ] After many quiet-link timeouts (several minutes idle), notifications still arrive. If not, switch `recv_timeout` from `SO_RCVTIMEO` to `select()` before `recv`.
- [ ] Phone contention: behaviour when the phone holds the v2 channel.

## 8. Phase 4 (owner present)

Run with the real headphones connected. Use `HAIRLINE_FRAME_LOG=1` if you also want the log from launch.

- [ ] Settings: turn on "Advanced panel"; the Advanced screen appears.
- [ ] The capability list shows the declared ids with their names (DECLARED chips).
- [ ] Press Record in the frame log; after a reconnect the log shows the handshake. Stop ends the file in `~/Library/Logs/com.drake0306.hairline/captures/`.
- [ ] Raw GET `66 17` (T1) returns a RET in the log, and it is sent without typing SEND.
- [ ] Raw SET on a declared function, for example volume `A8 20 <n>`: it is refused until `SEND` is typed, and the NTFY in the log confirms the change.
- [ ] A deny-listed raw (`24 03 01`) is refused even with `SEND`.
- [ ] Run the probe to completion (up to about 13 minutes, type `RUN`). Record how many functions answered and how many became proven, and attach the `~/Library/Logs/com.drake0306.hairline/captures/probe-<secs>.json` summary. Check `devices/<address>.json` lists the proven functions.
- [ ] Time left appears after an hour of listening (30 minutes and a 2 % drop at least), and Clear in Settings empties it.
- [ ] From a terminal, `hairlinectl state` and `hairlinectl battery` print the same values as the window.
- [ ] `hairlinectl volume 31` is refused with the same text as the window.

## 9. Phase 5 (owner present)

Run with the real headphones connected.

- [ ] Sound: press Advanced. Set a visible curve (for example +6 on the three lowest bands, 0 above), and page with ‹ and › through 1–4, 5–8 and 7–10.
- [ ] On Manual, press Send best fit to headphones. The dialog names "Manual" and lists six values; press Send. With the frame log on, the NTFY `59 03 A0 …` carries the six fitted values (raw = level + 10) and the ULT byte is the same as before the send.
- [ ] Choose Bright on the Sound screen. The faders fade, the note "Sony applies this preset inside the headphones and does not report its band values. Choose Manual, Custom 1 or Custom 2 to shape your own." appears under them, and dragging or arrow-keying a fader sends nothing (no `58 03` in the frame log). On Off the note reads "Equalizer off." and the faders are inert too.
- [ ] From Bright, open Advanced and press Send best fit to headphones. The dialog names "Custom 1 (the headphones will switch to it)". Press Cancel (and once Escape): nothing goes out. Press Send best fit again and confirm: exactly one `58 03 A1 <ULT> 06 …` goes out and the NTFY `59 03 A1 …` carries the fitted bands with the ULT byte unchanged; the Sound screen now shows Custom 1 with those six values.
- [ ] In Advanced, press Reset: the four faders and the best fit read 0, the curve survives a restart as all zeros, and nothing goes out in the frame log.
- [ ] At the default 880×560 window, the Advanced view does not scroll, the four faders are tall (about 300 px of travel) with clear space between them, and the bottom row (values, note, Reset, Send) is on one line.
- [ ] On the Sound screen with Bright selected at 880×560, the fixed-preset note fits under the faders with no clipping and nothing scrolls. In Advanced on Bright, "On the headphones now" reads "--" and the same note sits under the values.
- [ ] "On the headphones now" in the Advanced view matches the fit, and the Sound screen faders show the same six values.
- [ ] Back on the Sound screen, the fader knobs snap to the 21 segments and the +10, 0 and −10 labels are not clipped in the real window.
- [ ] Restart Hairline; the Advanced curve is still there (`eq_curve.json` in the config directory).

- [ ] Share codes (Phase 5c): on Custom 1 with a visible curve and ULT 1, press Share on the Sound screen. "Share this preset" shows a QR and a code starting with `S`; Copy says "Copied" and the clipboard holds the dashed code. Scanning the QR with a phone camera reads the same text. On Bright, Off, or disconnected, Share is unavailable.
- [ ] Change the curve and ULT, then (on another run, after a restart) press "Paste a code" and paste the copied code in lower case without dashes. The six values, "ULT 1" and "Custom 1" appear under the field; changing one character shows "That code has a typo (check failed).". Press Apply: the dialog "Apply this preset to the headphones?" opens and nothing has gone out yet (frame log). Cancel sends nothing. Apply again and confirm: exactly one `58 03 A1 01 06 <six bands>` goes out and the NTFY `59 03 A1 01 …` brings the Sound faders back to the shared values.
- [ ] In Advanced, Share a curve and paste its code back after a Reset: the ten bands return, `eq_curve.json` holds them, and no `58 03` goes out. A Sound code pasted in Advanced opens the same confirmation dialog. At 880×560 the Advanced view still does not scroll with the Share / Paste a code row above Send.

## 10. Windows app (Phase 7a, to do with a Windows machine)

The app builds and its tests pass on Windows CI; it has not been started on a Windows machine or run against the headphones on Windows. Nothing below is verified. Do section 7 (the transport) first.

- [ ] `HAIRLINE_FAKE=1`: the app starts, the tray icon appears in the notification area, and a left click opens the popover centred above the icon with the `FAKE` label.
- [ ] The tray icon is visible on both light and dark taskbars.
- [ ] The popover has no title bar, stays on top, and does not appear in the taskbar.
- [ ] With the taskbar on the left, right or top, the popover opens by the icon and fully on screen.
- [ ] Clicking outside hides the popover (set_focus takes the foreground, so focus loss arrives). Clicking the tray icon while it is open hides it and does not reopen it at once; the next click opens it.
- [ ] Right click on the tray icon shows Quit, and Quit ends the app.
- [ ] The popover footer opens the main window; it appears in the taskbar like an ordinary window, and closing it leaves the tray icon running.
- [ ] With the headphones paired and connected, the popover shows battery, noise control and ULT, and a change from the popover takes effect on the headphones.
- [ ] Devices: the Link list shows no host entry and no "playing" (the Windows audio module does not exist yet).
- [ ] Scenes: an "App in front" trigger with an executable name (for example `Zoom` for `Zoom.exe`) applies when that app is in front. A "Microphone in use" trigger never fires on Windows yet.
- [ ] Launch at login: turning it on in Settings starts Hairline after signing in again.
- [ ] Record the Windows version and the headphones' firmware with the results.

## 11. Linux (Phase 8a, to do with a Linux machine)

The transport and the app build and their tests pass on Linux CI (`ubuntu-latest`); nothing has been started on a Linux machine or run against the headphones on Linux. Nothing below is verified.

- [ ] Install the libraries from the README's Linux paragraph; `pnpm tauri dev` builds.
- [ ] `HAIRLINE_FAKE=1`: the app starts and the tray icon appears (GNOME needs the AppIndicator extension; KDE shows it in the system tray). The icon is readable on the panel.
- [ ] The tray menu shows "Show Hairline" and "Quit Hairline". "Show Hairline" opens the popover at the top-right of the screen with the `FAKE` label; it has no title bar, stays on top, and is not in the task list. Clicking outside hides it. Quit ends the app.
- [ ] The popover footer opens the main window; closing it leaves the tray icon running.
- [ ] Pair and connect the headphones with BlueZ (`bluetoothctl connect <address>` or the desktop's Bluetooth settings), then start Hairline without the fake. Note the `hairline: RFCOMM channel N (...)` line on stderr: which channel was chosen, and whether it came from the channel scan, the recorded channel or `HAIRLINE_RFCOMM_CHANNEL`. Expected: channel 18 on Roy's unit, from the scan on the first connect.
- [ ] If the scan ran: note how long it took (from start to the `RFCOMM channel` line). While it probes the channels before 18 (15 is expected to open, and 19, 20 and 22 if the scan gets past 18), watch the audio: music keeps playing, and on a call (HFP) the call audio and microphone are not interrupted or switched. Note anything that glitches and on which channel.
- [ ] `devices/<address>.json` in the config directory (`~/.config/com.drake0306.hairline/`) now holds `"rfcomm_channel": 18`. Quit and start again: the line says "recorded channel" and no scan runs.
- [ ] `HAIRLINE_RFCOMM_CHANNEL=18` is used first, without a probe. An invalid value (for example `99`) logs `ignoring HAIRLINE_RFCOMM_CHANNEL` and is not used. A valid but wrong channel that accepts connections (for example 15) is trusted, so the session fails and keeps retrying on it: unset the variable to recover. A wrong recorded channel behaves the same way: delete `rfcomm_channel` from `devices/<address>.json`.
- [ ] If the log says `no RFCOMM channel answered the MDR v2 protocol`: try `HAIRLINE_RFCOMM_CHANNEL=18`, or find the channel with `sdptool browse <address>` (needs `bluetoothd` started with `--compat`) by looking for the record with UUID `956C7B26-D49A-4BA8-B03F-B17D393CB6E2`, and note the channel it lists.
- [ ] With the headphones paired but not connected, Hairline logs "no connected device exposes the v2 service" and opens no socket; connecting them later is picked up by the retry.
- [ ] With the headphones connected, the popover shows battery, noise control and ULT, and a change from the popover takes effect on the headphones.
- [ ] Devices: the Link list shows no host entry and no "playing" (the Linux audio module does not exist yet). Scenes: a "Microphone in use" or "App in front" trigger never fires on Linux yet.
- [ ] `cargo install --path crates/mdr-cli`, then `hairlinectl state`, `hairlinectl nc ambient --level 12` and `hairlinectl battery` work against the running app.
- [ ] Launch at login: turning it on in Settings starts Hairline after signing in again.
- [ ] Record the distribution, desktop, BlueZ version (`bluetoothctl --version`) and the headphones' firmware with the results.
