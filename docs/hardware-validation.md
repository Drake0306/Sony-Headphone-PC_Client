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

## 4. Not yet done

- Controls from the popover (NC → Ambient → Off, level, voice focus, ULT 0/1/2) and the EQ-type experiment (`58 03` vs `58 00`) are write tests and wait for the owner's go-ahead; the current preset (0xA2, bands above) is recorded here so it can be restored.
- Phone-contention test (requires the phone).
- Menu-bar items that need eyes on the screen: tray icon, popover placement, hide on focus loss.
