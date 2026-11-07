# Phase 5c: Share codes and QR for equalizer presets — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A person can share an equalizer setting as a short code and a QR, and another person can paste that code into Hairline and have the values filled in. Works for the Sound screen's six-band curve (plus ULT) and for the Advanced ten-band curve.

**Architecture:** A pure TypeScript codec (`src/lib/presetCode.ts`) turns values into a 15-character code and back: Crockford base32 (digits and letters without I, L, O, U), shown as `XXXXX-XXXXX-XXXXX`. Character 1 is the kind (`S` = Sound: six raw bands 0..20 in 5 bits each, ULT in 2 bits, target slot in 2 bits; `A` = Advanced: ten bands −10..10 stored as 0..20 in 5 bits each), characters 2–13 are a 60-bit payload, characters 14–15 are a 10-bit CRC over the first 13 characters, so a typo or a swapped pair is rejected. The QR encodes the code text itself (no URL), drawn as SVG by the MIT `qrcode-generator` package bundled with the app. Sharing is a modal with the QR, the code in mono and a Copy button. Pasting is a modal with a field that accepts the code with or without dashes and in any case, shows the decoded values, and then applies them: an `S` code is written to the headphones only after the same confirmation dialog as "Send best fit" (target preset rule from Phase 5b) through a new `set_eq_preset_bands` command on the existing single-frame path; an `A` code is saved into the Advanced curve and opens the Advanced view. Nothing new goes on the wire.

**Spec:** owner's words of 2026-11-08: "share our own custom presets with others … a button when pressed, a QR is generated, and along with it a copy code … paste it … it will auto-select all the values … about 15 digit long, mixture of numbers and letters … same thing for advanced". `docs/superpowers/specs/2026-11-07-ult-wear-desktop-app-design.md` ("Blank, never invented"; deny-list and gating).

## Global Constraints

- No AI attribution; plain commits; full acceptance set (cargo test/clippy/fmt, Windows cross-check, `pnpm check`, `pnpm test` exit 0).
- The codec is deterministic and documented in `docs/protocol-notes.md`? No: in `README.md` "Sharing a preset" with the exact layout, so another tool could read the codes.
- A pasted code never writes to the headphones without the confirmation dialog; the write is the existing `58 03 <preset> <ult> 06 <bands>` frame via `Session::set_eq_preset_bands`; the target preset follows the Phase 5b rule (Manual/Custom 1/Custom 2 if current, else Custom 1) unless the code carries a slot (then that slot, still only A0/A1/A2).
- Decoding rejects: wrong length, characters outside the alphabet after normalisation (lower → upper, I/L → 1, O → 0), bad CRC, unknown kind, raw band > 20, ULT > 2, slot 3 is "no slot" (use the rule). Error text: "That code is not a Hairline preset." / "That code has a typo (check failed)."
- Tokens only; nothing static; the QR is rendered from the code at open time.

## Review Focus

1. A single-character typo or a swapped adjacent pair must be rejected — Task 1 `code_rejects_typos_and_swaps` (sweep every position with a different symbol; swap every adjacent pair).
2. Encode→decode round-trips every field for both kinds, including band extremes 0 and 20, ULT 0..2, slots 0..3 — Task 1 `code_round_trips`.
3. Pasting an `S` code must send nothing until the dialog's Send — Task 2 `pasted_code_waits_for_confirmation`.
4. Pasting an `A` code must save ten bands and send nothing — Task 2 `pasted_advanced_code_saves_only`.
5. Normalisation: `hl-abcde-...` lower case, with/without dashes, I/L/O mapped — Task 1 `code_normalises_input`.

---

### Task 1: Codec
**Files:** create `apps/desktop/src/lib/presetCode.ts` (+ `presetCode.test.ts`): `ALPHABET`, `encodeSound({bands: number[6] raw 0..20, ult: 0|1|2, slot: 0|1|2|3}) -> string` (formatted with dashes), `encodeAdvanced(bands: number[10] in −10..10) -> string`, `decode(text) -> { kind: "sound", bands, ult, slot } | { kind: "advanced", bands } | { error: string }`, `normalise(text)`, `crc10(bits)`; include two fixed test vectors (write the expected codes into the test after computing them once, and a comment with the bit layout) so the format cannot drift.
- [ ] Red → green → `pnpm check`/`pnpm test`. Commit `Add the preset share code codec`.

### Task 2: Share and paste in the UI, apply path
**Files:** `package.json` (+ `qrcode-generator`, pinned; check its licence is MIT and note it in `THIRD_PARTY.md`), create `src/lib/controls/ShareCode.svelte` (QR as inline SVG, the code in mono split by dashes, Copy → clipboard with "Copied"), `src/lib/controls/PasteCode.svelte` (field, live validation, decoded preview: six values with device labels + ULT + target, or ten values with frequencies; Apply button), modify `src/views/Sound.svelte` (buttons "Share" (enabled on Manual/Custom when connected) and "Paste a code" in the Presets block), `src/views/AdvancedEq.svelte` (buttons "Share" and "Paste a code" beside Reset), `src/lib/ipc.ts`; Rust: `apps/desktop/src-tauri/src/commands.rs` `set_eq_preset_bands({ preset, bands })` (preset must be A0/A1/A2 and listed by the capability; bands exactly the capability's count and ≤ steps−1) → `BrokerCmd::SetEqPresetBands`; register in `lib.rs`; test in `tests/eq_curve.rs` or a new `tests/share.rs` (fake: exact frame).
- [ ] Apply flows: `S` → confirmation dialog (reuse `Confirm.svelte`; title "Apply this preset to the headphones?"; body names the target and ULT) → `set_eq_preset_bands`; `A` → `set_eq_curve` + open Advanced. Tests per Review Focus 3–4 plus "Share from Sound encodes the current device bands and ULT with the current slot" and "Share from Advanced encodes the stored curve".
- [ ] Commit `Share and paste equalizer presets as codes and QR`.

### Task 3: Docs
- [ ] README "Sharing a preset" (how to share, how to paste, the exact code layout and alphabet, that the QR is just the code); `docs/hardware-validation.md` §9 additions (share from Custom 1, paste on another run, confirm the frame). Commit `Document preset sharing`.
