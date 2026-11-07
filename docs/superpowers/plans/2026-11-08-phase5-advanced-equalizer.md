# Phase 5: Advanced Equalizer — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A redesigned fader for the Sound screen (segments, zero line, readable end labels, the look from the research mock), and a full-area Advanced equalizer: ten bands, four visible at a time with paging, a curve stored in the app, a "Send best fit to headphones" action that writes the closest six-band match through the existing EQ command, and an explicit note that the ten bands run in Hairline only.

**Architecture:** One `Fader.svelte` replaces the current `Faders.svelte` internals and is used by both the Sound screen (six device bands) and the new `AdvancedEq.svelte` view (ten bands). Curves live in `eq_curves.json` beside `settings.json` (one active curve, serde, tmp+rename, same store pattern as scenes). The best fit is a pure function in `mdr-proto` (`eq::best_fit`) so the device mapping is tested where the band definitions live. Nothing new goes on the wire: the fit is written with the existing `set_eq_bands`. No audio engine in this phase (that is Phase 6, see `docs/research/reports/Advanced equalizer research.html`).

**Tech Stack:** as before. No new crates.

**Spec:** `docs/superpowers/specs/2026-11-07-ult-wear-desktop-app-design.md` (Sound screen; "Blank, never invented"); owner requests of 2026-11-08: segments like the phone (21 steps, −10..+10, zero line, you cannot land between segments), fix clipped "+10/−10" labels, adopt the advanced-EQ mock look; full-area view with back button, four big faders, paging strip with left/right buttons, label that these settings are not saved to the headphones; research note `docs/research/reports/Advanced equalizer research.html` (verdict, mock, pipeline).

## Global Constraints

- No AI attribution; plain commits; every commit passes the full acceptance set (cargo test/clippy/fmt, Windows check, `pnpm check`, `pnpm test`).
- Colours via tokens only. Accent is ULT orange; the paging strip uses the same accent unless the owner later picks a theme accent (ruling: no pink until the owner decides; the mock showed both).
- The device EQ stays six values with 21 steps each; the Advanced curve is ten bands 31, 62, 125, 250, 500 Hz, 1, 2, 4, 8, 16 kHz, 21 steps each (−10..+10, integers only, no in-between positions).
- Best fit: Clear Bass = rounded mean of the 31/62/125/250 Hz values; 400 Hz = interpolate 250↔500 in log frequency; 1 kHz = 1 kHz; 2.5 kHz = interpolate 2↔4; 6.3 kHz = interpolate 4↔8; 16 kHz = 16 kHz; round half away from zero; clamp to −10..10. Deterministic, tested with fixtures.
- The note shown in the Advanced view, verbatim: "These bands run in Hairline only. The headphones keep their six-band curve; use Send best fit to put the closest match on them."
- The Advanced view never writes to the device by itself; only the Send best fit button does, through the existing control with the normal pending/confirmed/refused states.
- Nothing static: the fitted six values shown next to the button are computed from the curve; the device's current six values come from state.

## Review Focus

1. A fader must never report a value between segments or outside −10..10 from any pointer position — Task 1 `fader_snaps_to_segments_and_clamps`.
2. Keyboard users must be able to move a fader (ArrowUp/Down ±1, Shift ±5, Home/End) — Task 1 `fader_keyboard_steps`.
3. Best fit must reproduce a six-band curve that is already flat/known: a curve that is 0 everywhere fits to all zeros; a curve with +6 at 31..250 and 0 above fits Clear Bass +6 and the rest 0 — Task 2 `best_fit_fixtures`.
4. Paging must never show an empty page and must keep the selected fader's focus — Task 3 `paging_bounds`.
5. Leaving the Advanced view with unsaved edits must not lose the curve (auto-saved to the app store on every change) — Task 3 `curve_persists_on_change`.

---

### Task 1: Fader redesign (shared control)

**Files:**
- Create: `apps/desktop/src/lib/controls/Fader.svelte` (+ `Fader.test.ts`)
- Modify: `apps/desktop/src/lib/controls/Faders.svelte` (becomes a thin row of `Fader`s), `apps/desktop/src/views/Sound.svelte` (labels), `apps/desktop/src/styles/base.css` (fader classes)

**Interfaces:** `Fader` props `{ value: number | null, label: string, status: Status, disabled?: boolean, onchange: (v: number) => void, oncommit: (v: number) => void }`; 21 tick marks (`.seg`, zero tick stronger), fill from zero to the knob (`.fill`, `.fill.neg`), knob `var(--ult)`, value in Doto above the label, end labels "+10" / "−10" rendered inside the track column with room (no clipping at 32 px columns: labels go in a `.ends` row below with `white-space: nowrap`), pointer drag snaps to the nearest segment, keyboard steps per Review Focus 2, `role="slider"` with `aria-valuemin/max/now`.

- [ ] Failing tests: `fader_snaps_to_segments_and_clamps`, `fader_keyboard_steps`, `fader_renders_21_segments_and_zero_line`, `sound_faders_show_all_end_labels` (Sound renders six faders with "+10" and "−10" visible, not clipped — assert the elements exist and have no `overflow: hidden` ancestor cutting them; use a snapshot of class names).
- [ ] Implement; keep `Faders.svelte`'s external props so Sound.svelte changes are minimal.
- [ ] Commit `Redesign the equalizer fader with segments and a zero line`.

---

### Task 2: Curve store and best fit

**Files:**
- Create: `crates/mdr-proto/src/eq.rs` (`ADVANCED_BANDS_HZ: [u32; 10]`, `DEVICE_BANDS: [DeviceBand; 6]`, `pub fn best_fit(curve: &[i8; 10]) -> [i8; 6]`), `apps/desktop/src-tauri/src/eq_curve.rs` (`EqCurve { bands: [i8; 10] }`, `load/save` in the settings dir), commands `get_eq_curve`, `set_eq_curve(bands)` (validates −10..10), `send_best_fit()` (computes the fit, converts to the device's 0..20 scale, sends `BrokerCmd::SetEqBands` through the existing path)
- Test: `crates/mdr-proto/src/eq.rs` unit (`best_fit_fixtures`, `best_fit_rounds_half_away_from_zero`, `best_fit_clamps`), `apps/desktop/src-tauri/tests/eq_curve.rs` (`round_trip`, `rejects_out_of_range`, `send_best_fit_uses_set_eq_bands` on the fake: the frame written is `58 03 <preset> <ult> 06 <6 bands>` with the fitted values and the current preset/ULT preserved)

- [ ] Red → green → full checks. Commit `Store the advanced curve and fit it to the device bands`.

---

### Task 3: Advanced equalizer view

**Files:**
- Create: `apps/desktop/src/views/AdvancedEq.svelte` (+ test), `apps/desktop/src/lib/eqCurve.ts` (store + helpers `pageOf(index)`, `visibleBands(page)`, `bestFitPreview(curve)` mirroring `best_fit` for display with a parity test against fixtures)
- Modify: `apps/desktop/src/views/Sound.svelte` (an "Advanced" `.btn` beside the equalizer header), `apps/desktop/src/views/Main.svelte` (the view takes the whole content area; the sidebar nav hides while it is open; back button "‹ Sound" top left returns), `src/lib/ipc.ts`

- [ ] Layout per the research mock: title row (back, "Advanced equalizer", "Bands n–m of 10"), four `Fader`s in a 4-column grid filling the height, paging strip (ten segments, visible four in accent) with ‹ › buttons stepping by four (pages 1–4, 5–8, 9–10 with the last page right-aligned so it still shows two faders and never an empty page), the verbatim note in accent, "Send best fit to headphones" button with the fitted six values shown beside it as small Doto numbers and the normal pending/refused states; edits auto-save through `set_eq_curve`.
- [ ] Tests: `paging_bounds`, `curve_persists_on_change`, `send_best_fit_invokes_command`, back returns to Sound.
- [ ] Commit `Add the Advanced equalizer view`.

---

### Task 4: Docs

- [ ] README "Advanced equalizer" section (what is saved where; the honest limitation; the fit rule in one sentence); Status: Phase 5 done, "Next: the Windows tray app, then Linux (the in-app audio engine is Phase 6, undecided)"; `docs/hardware-validation.md` §9: send a best fit and confirm the NTFY echoes the six fitted values; `docs/protocol-notes.md`: nothing new on the wire. Commit `Document the advanced equalizer`.
