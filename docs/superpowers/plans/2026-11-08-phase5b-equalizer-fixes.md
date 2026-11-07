# Phase 5b: Equalizer fixes from live use — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Four fixes the owner found using the app with the headphones connected on 2026-11-08: (1) fixed Sony presets show every band at 0 and Off does not disable the faders; (2) the Advanced equalizer's four faders look squashed in the 880×560 window; (3) "Send best fit to headphones" must ask first and say exactly what it will do and into which preset; (4) the Advanced equalizer needs a Reset button.

**Architecture:** No new wire commands. (1) is honest presentation: the headphones apply fixed presets internally and report centre values for them (verified from the live state, which the Sound screen shows unchanged), so the faders become read-only on fixed presets with a note, and disabled with a note when the preset is Off; nothing is invented. (2) is CSS: the fader track takes the free height, the bottom block becomes compact. (3) adds an in-app confirmation (Hairline style; the webview has no `alert`/`confirm`) and a single-frame write that sets the target preset and the six bands together (`58 03 <preset> <ult> 06 <bands>`, preserving ULT), through a new `Session::set_eq_preset_bands`. (4) is a local action on the stored curve.

**Spec:** owner's words of 2026-11-08 (below, verbatim where it matters); `docs/superpowers/specs/2026-11-07-ult-wear-desktop-app-design.md` "Blank, never invented"; `docs/protocol-notes.md` ULT/EQ section (read-modify-write; `58 03` carries preset, ULT and bands in one payload).

Owner: "in the presets, sound presets like bright, excited, relaxed, we don't actually have the values for it … everything is set to 0 … when we set it to off, then the equalizer should be disabled"; "on the advanced version … the four sound selected bars are looking squashed … create some space … expand it a bit"; "when somebody says send it to the headphones, don't just send it directly … give them a pop-up telling them what it's going to do, headphones doesn't support it … and in which preset is it going to set these current values … then the person selects yes, then only send it"; "add a reset button on the advanced equalizer … reset it back to zero".

## Global Constraints

- No AI attribution; plain commits; full acceptance set (cargo test/clippy/fmt, Windows + Linux cross-checks of mdr-transport/mdr-cli, `pnpm check`, `pnpm test` exit 0).
- Nothing invented: band values shown are always the device's; the fixed-preset note explains why they read 0. Tokens only.
- The only new device write is `send_best_fit`'s single `58 03` frame; the deny-list and gating are unchanged; Reset never touches the device.
- Target preset rule: if the current preset is Manual (`A0`), Custom 1 (`A1`) or Custom 2 (`A2`), the fit is written into that preset; otherwise into Custom 1 (`A1`), and the dialog says the headphones will switch to it.
- Dialog copy (verbatim): title "Send best fit to the headphones?"; body "The headphones hold six bands (Clear Bass, 400 Hz, 1 kHz, 2.5 kHz, 6.3 kHz, 16 kHz), not ten. Hairline fits your ten bands onto those six and writes them into {target}." where {target} is "Manual" / "Custom 1" / "Custom 2" / "Custom 1 (the headphones will switch to it)"; then the six fitted values with their labels; buttons "Send" and "Cancel". Escape cancels.
- Fixed-preset note (verbatim): "Sony applies this preset inside the headphones and does not report its band values. Choose Manual, Custom 1 or Custom 2 to shape your own." Off note: "Equalizer off."
- Layout target: in the 880×560 window the Advanced fader track is at least 300 px tall, the four faders have at least 24 px between columns, the strip and bottom block fit without scrolling, and nothing overlaps.

## Review Focus

1. Dragging a fader on a fixed preset must send nothing — Task 1 `fixed_preset_faders_are_inert`.
2. Send must write exactly one frame carrying the target preset and the six fitted bands with the current ULT preserved — Task 1 `send_best_fit_writes_preset_and_bands_in_one_frame` (fake: current preset `0x10` Bright, ULT 2 → frame `58 03 A1 02 06 …`).
3. Cancelling the dialog must send nothing — Task 1 `cancel_sends_nothing`.
4. Reset must save ten zeros and send nothing — Task 1 `reset_zeroes_and_saves`.
5. The layout rule above must be expressed in CSS that a test can check (no fixed track height; `min-height` on the track; compact bottom) — Task 1 `advanced_layout_rules`.

---

### Task 1: All four fixes

**Files:** `apps/desktop/src/views/{Sound,AdvancedEq}.svelte`, `src/lib/controls/{Fader,FaderScale,Faders}.svelte` (size via CSS variables, no fixed track height for the `lg` size), new `src/lib/controls/Confirm.svelte` (modal: ink border, paper panel, `.btn` + `.btn.fill`, focus trapped, Escape cancels), `src/lib/eqCurve.ts` (`targetPreset(current: number | null) -> { id, label, switches: boolean }`, `resetCurve()`), `src/lib/ipc.ts`; Rust: `crates/mdr-session/src/session.rs` (`set_eq_preset_bands(preset: u8, bands: Vec<u8>)`), `apps/desktop/src-tauri/src/{eq_curve,commands,broker}.rs` (`send_best_fit({bands, preset})` → `BrokerCmd::SetEqPresetBands(preset, raw)`), tests in `crates/mdr-session/tests/gating.rs` or `tests/eq_curve.rs`, `src/views/*.test.ts`.
- [ ] Red → green → full checks. Commit `Fix the equalizer from live use: honest presets, roomier Advanced view, confirmed send, reset`.

### Task 2: Docs
- [ ] README (Advanced equalizer section: the dialog, the target-preset rule, Reset; Sound: fixed presets read-only); `docs/hardware-validation.md` §9 additions (send into Custom 1 from Bright and confirm `59 03 A1 …`; Reset; the fixed-preset note appears). Commit `Document the equalizer fixes`.
