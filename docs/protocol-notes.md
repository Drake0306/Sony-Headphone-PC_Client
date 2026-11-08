# MDR v2 protocol notes for the ULT WEAR subset

Opcode values and payload layouts below were read on 2026-11-07 from mos9527/SonyHeadphonesClient `libmdr/include/mdr/Generated/ProtocolV2T1.hpp`, `ProtocolV2T2.hpp` and `ProtocolV2.hpp` (branch `v1-compat`, MIT; see `THIRD_PARTY.md`). Live-verified byte strings come from `docs/research/notes/Sony headphone PC client comparison/ult_wear_support.md` (owaiskaifi, sony-ult-ctl, Z-ULT-Connect, pats01, SonyBridge, all on WH-ULT900N fw 2.0.7 or 2.1.0). Anything marked **verify** has not been seen on hardware yet.

## Transport and framing

- Service UUID (v2): `956C7B26-D49A-4BA8-B03F-B17D393CB6E2`, RFCOMM, channel via SDP (18 on Roy's unit). Only one client at a time.
- Linux channel scan (BlueZ has no SDP browsing over D-Bus): `SHDC_RFCOMM_CHANNEL`, then the channel recorded per unit, both used without a probe as soon as they connect (the session's handshake proves them); then the other channels 1..=30, skipping any that refuse, are busy or take over 5 s to connect. Each scanned channel that accepts the connection is sent only the T1 protocol-info GET `00 00` (seq 1) and is kept on a v2 reply (`01 00 …`, 8 bytes) within 1.5 s, else closed. GET-only, so the deny-list cannot be reached; the reply is handed to the session, which ACKs it and runs its own handshake.
- Frame: `3E | type | seq | len u32 BE | payload | checksum | 3C`. Escape `3C/3D/3E` inside the inner bytes as `3D, (b & 0xEF)`. Checksum = low 8 bits of the sum of `type, seq, len bytes, payload` (everything between the `3E` and the checksum, after unescaping). Types: `01` ACK, `0C` COMMAND_1 (table 1), `0E` COMMAND_2 (table 2).
- ARQ: every data frame must be ACKed with `seq = 1 - received_seq`; one frame in flight; the device retransmits forever if not ACKed. Our own sends wait for the device's ACK before the next send.
- Primitive encodings in payloads: `MDRPodArray<T>` = one byte count then elements; `MDRArray<T>` = one byte count then variable-size elements; `MDRPrefixedString` = one byte length then bytes (UTF-8, not NUL-terminated). Integers big-endian.

macOS (IOBluetooth, hardware-checked on the WH-ULT900N fw 2.1.4, macOS 27.2): `openRFCOMMChannelSync` called inside a main-queue dispatch block returns `kIOReturnError` (0xe00002bc) after about 1.5 s on a fresh link, every time, because it needs to pump the run loop itself. It also leaves the v2 DLCI half-open on the device, so later opens on that channel hang (no UA, no DM) until the device times it out. The same synchronous call made directly on the main thread succeeds (about 2.7 s), and `openRFCOMMChannelAsync` started on the main queue succeeds in about 1.3 s with `rfcommChannelOpenComplete:status:` status 0 on the main run loop. The transport therefore opens asynchronously and waits for that callback off the main thread (15 s limit). A stuck DLCI shows as an open that never completes (no UA, no DM); a non-zero open status or a close before the open completes means the device answered and the DLCI is not stuck. Only after a timed-out open does the transport call `closeConnection` on the device, which resets the baseband link and clears the stuck DLCI; after `closeConnection` macOS does not re-establish the link on its own; call `openConnection` to restore it (about 1–2 s). The transport waits about 1.5 s and then calls `openConnection`; a later async open on the v2 channel then succeeds immediately. Other RFCOMM channels on the device (15, 19, 20, 22) open either way.

## Table 1 opcodes (COMMAND_1) used

| Group | GET | RET | SET | NTFY | Capability GET/RET |
|---|---|---|---|---|---|
| Connect | `00` protocol info, `02` capability info, `04` device info, `06` support functions | `01`, `03`, `05`, `07` | | | |
| Common (codec, DSEE indicator) | `12` status | `13` | `18` param | `15` status, `19` param | `10`/`11` |
| Power (battery, power off, auto off) | `22` status, `26` param | `23`, `27` | `24` status, `28` param | `25`, `29` | `20`/`21` |
| EQ/EBB (EQ, ULT) | `56` param | `57` | `58` | `59` | `50`/`51`, extended `5A`/`5B` |
| NC/ASM | `66` param | `67` | `68` | `69` | `60`/`61` |
| Play (volume, playback) | `A2` status, `A6` param | `A3`, `A7` | `A4`, `A8` | `A5`, `A9` | `A0`/`A1` |
| Log | | | `C4` set status (**never send**) | `C9` notify | |
| General settings (D1..D3) | `D2` status, `D6` param | `D3`, `D7` | `D8` | `D5`, `D9` | `D0`/`D1` |
| Audio (DSEE auto-off, connection mode) | `E2` status, `E6` param | `E3`, `E7` | `E8` | `E5`, `E9` | `E0`/`E1` |
| System (voice assistant, quick access, reset, wear pause) | `F2` status, `F6` param, `FA` ext | `F3`, `F7`, `FB` | `F4`, `F8`, `FC` | `F5`, `F9`, `FD` | `F0`/`F1` |

Table 2 (COMMAND_2) used: `06`/`07` support functions; voice guidance `40..48` (`46` get param, `47` ret, `48` set; inquired type `20` VOLUME, `Int8` −2..2); safe listening `50..5B`; peripheral (paired device management) `30..3D` (**verify**: the ULT WEAR declares no PERI function, multipoint on this model is General Setting 1).

## Deny-list (never transmitted outside explicit, confirmed Advanced actions)

`F8` with type `09` (RESET_SETTINGS; `F8 09 01` = factory reset), `98` (ALERT_SET_PARAM family, used for reboot/commit sequences), `24` (POWER_SET_STATUS; power off), `2D`/`30..3F` (UPDT firmware), `C4` (LOG_SET_STATUS). Note `98 00 06 01` is part of the multipoint-toggle sequence reported by Gadgetbridge/SonyBridge; it is allowed only inside that sequence.

## Capability gating

Every typed request names the support function it needs. The session sends it only if the headphones declared that function in the support-function reply. A function that is only *proven* (it answered the probe although it was not declared, stored per unit) satisfies the check for GETs only, by the opcode role table (`mdr_proto::function::opcode_role` is `Get`); a typed SET on a proven function is refused as unavailable and nothing is written. Raw requests from the Advanced panel name no function and are not gated: they are deny-listed like everything else, and anything but a GET needs the typed `SEND` confirmation.

## Handshake

1. T1 `00 00` → `01 00 03 00 20 02 00 00` (`ConnectRetProtocolInfo`: type, protocolVersion i32 BE = 0x03002002, table1 enable, table2 enable). Reply length 8 ⇒ v2.
2. T1 `06 00` → `07 00 <n> (<fn> <priority>)×n`; T2 `06 00` → same shape. Roy's unit: T1 `10 12 13 14 15 20 23 25 27 32 53 69 6B 70 93 F4 90 A1 F1 B2 B7 C1 C2 D1 D2 D3 E3 E2 F9 FD`, T2 `42 50 F2`.
3. T1 `04 01` → `05 01 <len> "ULT WEAR"` (model name); `04 02` → `05 02 <len> "2.1.0"` (firmware); `04 03` → `05 03 <series> <color>` (**verify** on this unit).

Support function bytes (T1 FunctionType) relevant to us: `12` CODEC_INDICATOR, `13` UPSCALING_INDICATOR, `20` BATTERY_LEVEL_INDICATOR, `23` POWER_OFF, `25` AUTO_POWER_OFF_WITH_WEARING_DETECTION, `53` PRESET_EQ_AND_ULT_MODE, `69` AMBIENT_SOUND_CONTROL_MODE_SELECT, `6B` MODE_NC_ASM_…DUAL…LEVEL_ADJUSTMENT, `70` AUTO_NCASM, `A1` PLAYBACK_CONTROLLER_WITH_CALL_VOLUME_ADJUSTMENT, `C1`/`C2` log notifiers, `D1`–`D3` GENERAL_SETTING_1..3, `E2` UPSCALING_AUTO_OFF, `E3` CONNECTION_MODE_…_LDAC_STATUS, `F1` PLAYBACK_CONTROL_BY_WEARING_REMOVING_HEADPHONE_ON_OFF, `F4` VOICE_ASSISTANT_SETTINGS, `F9` RESET_SETTINGS, `FD` QUICK_ACCESS. T2: `42` VOICE_GUIDANCE (MTK transfer, language switch, volume), `50` SAFE_LISTENING_HBS_1, `F2` QUICK_ACCESS_EASY_SETTING.

## Battery

- GET `22 00` → RET `23 00 <level 0..100> <charging: 00 not / 01 charging / 02 unknown / 03 charged>`. No threshold byte on this model (BATTERY_WITH_THRESHOLD not declared). NTFY `25 00 …` same shape. Verified: `23 00 43 00` = 67 %.

## Noise control (NC / Ambient / Off)

- Inquired type `0x17` MODE_NC_ASM_DUAL_NC_MODE_SWITCH_AND_ASM_SEAMLESS (declared via `6B`).
- GET `66 17` → RET `67 17 <valueChange 00 under-changing / 01 changed> <ncAsmTotalEffect 00 off / 01 on> <ncAsmMode 00 NC / 01 ASM> <ambientSoundMode 00 normal / 01 voice> <level 00..14>`.
- SET `68 17 <valueChange> <total> <mode> <asmMode> <level>` (7 bytes incl. opcode and type). Off = total `00`. NC = total `01`, mode `00`. Ambient = total `01`, mode `01`, asmMode, level 1..20. Ambient fields are ignored while mode is NC; the firmware keeps the voice-focus bit while Off.
- NTFY `69 17 …` same shape as RET. The 8-byte `0x19` layout (adds ncAsmOnOffValue, noiseAdaptiveSensitivity) is for XM6-class devices; this model ignores it.

## ULT and EQ

- Inquired type `0x03` PRESET_EQ_AND_ULT_MODE (declared via `53`).
- CAP `50 03` → `51 03 <band 06> <step 15> <ult2AdditionalStep 0A> <nPresets 0C> (<presetId> <len> <name>)×n`.
- GET `56 03` → `57 03 <presetId> <ult 00/01/02> <nBands 06> <b1..b6>` (`EqParamUltMode`; bands centred at `0A`, 21 steps ⇒ −10..+10, index 0 = Clear Bass). Verified: `57 03 00 02 06 0A 0A 0A 0A 0A 0A`.
- SET `58 03 <presetId> <ult> 06 <b1..b6>` → ACK then NTFY `59 03 …`. Always read-modify-write: echo preset and bands from the last RET and change only what the user touched. **Disputed:** whether type `03` honours the preset/band bytes (owaiskaifi: ULT-only; Z-ULT-Connect: presets must go through `58 03`). Phase 1 hardware step: try both `58 03` and `58 00` (PRESET_EQ) and record which changes the preset.
- Preset ids: `00` OFF, `10` BRIGHT, `11` EXCITED, `12` MELLOW, `13` RELAXED, `14` VOCAL, `15` TREBLE, `16` BASS, `17` SPEECH, `A0` CUSTOM (manual), `A1..A5` USER_SETTING1-5 (**verify** that `A0`/`A1`/`A2` correspond to Manual / Custom 1 / Custom 2 in the capability name list).
- Extended `5A 00` → `5B 00 <n> (<infoType> <freq>)×n` gives band centre frequencies (**verify** on this unit; expected Clear Bass + 400/1k/2.5k/6.3k/16k).
- The Advanced equalizer's ten-band curve is app-side only (`eq_curve.json`); the only EQ write is the six-band `58 03` above, so "Send best fit" adds nothing to the wire.
- A malformed SET (`58 00 00 00 06` with no bands) corrupted the custom slot on a real unit. Validate lengths before sending.

## Codec, DSEE, connection mode

- Codec: GET `12 02` → RET `13 02 <codec>`; NTFY `15 02 <codec>`. Codec: `01` SBC, `02` AAC, `10` LDAC, `30` LC3, `00` unsettled.
- DSEE indicator: `12 03` → `13 03 <effectType> <status>`; auto-off setting via Audio: GET `E6 01` → `E7 01 <UpscalingTypeAutoOff>`; SET `E8 01 <value>` (**verify** value enum on hardware; declared via `E2`).
- Connection mode with LDAC status (`E3`): Audio inquired type `02` CONNECTION_MODE_WITH_LDAC_STATUS. GET `E6 02` → `E7 02 …`; SET `E8 02 <sound quality / stable>`; status `E2 02` → `E3 02 …` (**verify** layout; grep `AudioRetParamConnectionModeWithLdacStatus`-like structs in ProtocolV2T1.hpp, the generated name differs).

## Volume and playback

- Verified on this unit: GET `A6 20` → `A7 20 <vol 0..30>`; SET `A8 20 <vol>`; NTFY `A9 20 <vol>`. Synced with AVRCP absolute volume (0..127 ↔ 0..30).
- Playback status (`A1` PLAYBACK_CONTROLLER_WITH_CALL_VOLUME_ADJUSTMENT): `A2 01` → `A3 01 …` with playback status and track metadata (**verify**; useful for "playing" indicator).

## Multipoint (two-device connection) — General Setting 1

- Reported by Gadgetbridge (ULT900N commits) and SonyBridge golden fixture: GET `D6 D1` → RET `D7 D1 00 <value>` where settingType `00` = boolean and value `00` = ON, `01` = OFF (inverted enum: `GsSettingValue::ON = 0x00`). SET `D8 D1 00 <value>` followed by `98 00 06 01`. **verify** the trailing `98` frame is required on this model; if the device NTFYs `D9 D1 …` without it, drop it.
- Capability `D0 D1` → `D1 D1 <settingType> <stringFormat> <subject str> <summary str> [elements]` self-describes each general setting (D1..D3). Read all three at connect and show them generically; expect multipoint, touch panel, and a service-link style setting.
- Which paired device is "playing" and "take audio": the ULT WEAR declares no PERI function on T2, so the playing device comes from the host side (which of our audio outputs is active) plus `A3 01` playback status. "Take audio" = make this Mac the active A2DP source (host-side), not a headphone command, on this model.

## Power and wearing

- Power off: `24 03 01` (POWER_SET_STATUS, POWER_OFF, USER_POWER_OFF). Behind confirmation.
- Auto power-off with wearing detection (`25`): GET `26 05` → `27 05 <current> <lastSelected>`; SET `28 05 <current> <lastSelected>`. Values: `00` 5 min, `04` 15 min, `01` 30 min, `02` 60 min, `03` 180 min, `10` when removed from ears, `11` disabled.
- Pause when taken off (`F1`): System inquired type `01` PLAYBACK_CONTROL_BY_WEARING: GET `F6 01` → `F7 01 <on/off>`; SET `F8 01 <on/off>` (**verify** value enum; OnOffSettingValue ON=0x00? check `OnOffSettingValue` in ProtocolV2.hpp before use).

## System settings

- Voice assistant (`F4`): `F6 04` → `F7 04 <VoiceAssistant>`; SET `F8 04 <value>`. Values `30` voice recognition (phone), `31` Google, `32` Alexa, `34` Sony, `FF` none. Capability `F0 04` lists what the device offers.
- Quick Access (`FD`): `F6 0D` → `F7 0D <n> <fn…>`; SET `F8 0D <n> <fn…>`; capability `F0 0D` → `F1 0D …` with `QuickAccessFunctionInfo` (action, default, function list). T2 `F2` easy setting (**verify**).
- Reset (`F9`): `F8 09 00` settings only, `F8 09 01` factory reset. Deny-listed; Advanced only, double confirmation.
- Voice guide volume (T2 `42`): `46 20` → `47 20 <Int8 −2..2>`; SET `48 20 <v>`.

## Things to probe later (Advanced → opcode probe)

GET-only walk of declared and undeclared inquired types: `70` AUTO_NCASM wire format, `C9` action logs (passive), Power types `0B..0E` (battery safe mode, caring charge, BT standby, stamina), Audio `00`/`02`/`05`, T2 safe listening `56 …`. Never SET during a probe.

## Probe catalogue

The probe walks `GET_CATALOGUE` in `crates/mdr-proto/src/probe.rs`: 46 GET opcodes with the inquired types libmdr defines for each, 441 requests. It is GET-only; `probe_points` re-checks each entry against the opcode role table and the deny-list, so a wrong edit cannot make it send a SET. Excluded reads: the protocol-info and support-function requests (the handshake already sent them), System RESET_SETTINGS (nothing near a reset is sent, not even its read), and T2 LEA GET_IDENTITY_RESOLVING_KEY (the reply is a pairing secret). A function that answers although it was not declared is "proven" and stored per unit in `devices/<address>.json`, never globally.
