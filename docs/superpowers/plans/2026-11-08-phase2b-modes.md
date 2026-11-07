# Phase 2b: Modes (Scenes) — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Named modes (the owner's "per-activity levels", Sony's Adaptive Sound Control rebuilt for the desktop): each mode holds a noise/ambient/voice-focus/ULT/EQ/volume configuration, can be applied by hand from the Scenes screen, the popover or a hotkey, and can be applied automatically from desktop triggers (headphones connected, time of day, frontmost app, microphone in use). First match wins.

**Architecture:** A `scenes` module in the Tauri backend owns the list (persisted as `scenes.json` beside `settings.json`), a small evaluator that turns trigger inputs into "which scene is active", and macOS sensors for frontmost app and microphone-in-use. The broker thread applies a scene's actions through the existing session setters when the active scene changes or on a manual apply. The frontend gets the Scenes screen from the mockup (list + editor), a Mode row in the popover, and global hotkeys via `tauri-plugin-global-shortcut`. The headphones only ever receive the same commands the popover already sends; nothing new goes on the wire.

**Tech Stack:** as Phase 2, plus `tauri-plugin-global-shortcut = "2"`, `objc2-app-kit` (NSWorkspace frontmost app), `coreaudio-sys` (default input device running flag).

**Spec:** `docs/superpowers/specs/2026-11-07-ult-wear-desktop-app-design.md` (Scenes in the Architecture section and in "Features Sony never gave"), owner direction of 2026-11-07 ("make sure it changes per mode"), design `docs/research/reports/Hairline design screens.html` (Scenes section) and `docs/screenshots/06-scenes.png`.

## Global Constraints

- No AI attribution anywhere; commits as the repo identity, no trailers; every commit passes `cargo test --workspace`, clippy `-D warnings`, `cargo fmt --all -- --check`, `pnpm check`, `pnpm test`.
- A scene never bypasses the session: actions map 1:1 to existing setters (`set_nc`, `set_ult`, `set_eq_preset`, `set_volume`, `set_dsee_auto_off`), so gating, the deny-list and the four control states apply unchanged.
- "Keep" for any action field = `None` = leave that setting as it is.
- Scene data model (serde, `scenes.json`, pretty): `Scene { id: String (uuid v4), name: String, enabled: bool, trigger: Trigger, actions: Actions, hotkey: Option<String> }`; `Trigger = Manual | HeadphonesConnected | TimeRange { from: "HH:MM", to: "HH:MM" } | FrontmostApp { name: String } | MicInUse`; `Actions { nc: Option<"Off"|"NoiseCancelling"|"Ambient">, ambient_level: Option<u8 1..=20>, voice_focus: Option<bool>, ult: Option<0|1|2>, eq_preset: Option<u8>, volume: Option<u8 0..=30>, dsee_auto_off: Option<bool> }`.
- Evaluation: inputs `Inputs { connected: bool, now_minutes: u16 (local), frontmost_app: Option<String>, mic_in_use: bool }`; the active scene is the first enabled scene in list order whose trigger matches; `Manual` never matches automatically; `TimeRange` wraps midnight when `to < from`; app names match case-insensitively on the app's localized name.
- Apply policy: actions are sent when the active scene id changes (including from none → some) and on `apply_scene(id)`; never re-sent on every tick; a scene with no matching trigger does not "undo" anything when it stops matching (no implicit revert) unless a later scene matches.
- Sensors poll every 2 s on the broker thread's loop; failures degrade to `None`/`false` and are logged once.
- Hotkeys: the recorded string is Tauri's accelerator format (e.g. `Ctrl+Alt+1`); registration failures surface as a refused state on the Hotkey row with the error text; registered only while the app runs.
- Popover: a "MODE" row of boxed buttons listing enabled scenes by name (max 4, more → first 4 plus "…"), the active one filled; clicking applies that scene by hand (`apply_scene`); rendered only when at least one scene exists.
- Default seed when `scenes.json` is missing: four disabled examples modelled on Sony's app so the screen is not empty: "At the desk" (HeadphonesConnected → Ambient 14, voice focus on), "On a call" (MicInUse → NC), "Late" (TimeRange 22:00–07:00 → NC, volume 12), "Walking" (Manual → Ambient 20, ULT 1). Seeds are marked `enabled: false` except none; the owner enables what they want.

## Review Focus

1. Two scenes matching at once must apply only the first in list order, and reordering must change the outcome — pinned in Task 1 (`first_match_wins`, `reorder_changes_active`).
2. A tick where nothing changed must send no frames — pinned in Task 2 (`unchanged_active_scene_sends_nothing`).
3. Hotkey registration conflicts (already taken by the OS/another app) must not crash and must show as refused — pinned in Task 4 (`hotkey_conflict_is_refused`).
4. Deleting the active scene must clear the active id and not send a revert — pinned in Task 1 (`delete_active_clears`).
5. A scene whose action is refused by the headphones (e.g. Unavailable) must leave the other actions applied and the refusal visible on the corresponding control — pinned in Task 2 (`partial_refusal_keeps_rest`).

---

### Task 1: `scenes` core — model, store, evaluator (Rust, no Tauri types)

**Files:**
- Create: `apps/desktop/src-tauri/src/scenes/mod.rs`, `scenes/model.rs`, `scenes/store.rs`, `scenes/eval.rs`
- Test: `apps/desktop/src-tauri/tests/scenes_eval.rs`, `tests/scenes_store.rs`

**Interfaces:**
- Produces: the serde types from Global Constraints (`Scene`, `Trigger`, `Actions`, `Inputs`); `pub fn active_scene<'a>(scenes: &'a [Scene], inputs: &Inputs) -> Option<&'a Scene>`; `pub fn load(dir: &Path) -> Vec<Scene>` (missing → `seed_scenes()`, corrupt → seeds + log), `pub fn save(dir: &Path, scenes: &[Scene]) -> io::Result<()>`; `pub fn seed_scenes() -> Vec<Scene>`; `pub fn time_in_range(now: u16, from: u16, to: u16) -> bool`; `pub fn parse_hhmm(s: &str) -> Option<u16>`.

- [ ] **Step 1: Failing tests** — `first_match_wins` (two enabled scenes both matching `HeadphonesConnected`; active is the first), `reorder_changes_active`, `disabled_scene_never_matches`, `manual_never_matches_automatically`, `time_range_wraps_midnight` (`22:00–07:00` matches 23:30 and 06:00, not 12:00), `frontmost_app_case_insensitive`, `mic_in_use_matches`, `delete_active_clears` (helper `active_id_after_delete`), store: `load_seeds_when_missing` (4 scenes, all disabled, names as listed), `round_trip`, `corrupt_falls_back_to_seeds`.
- [ ] **Step 2: Run** `cargo test -p hairline --test scenes_eval --test scenes_store` → compile errors. **Step 3: Implement.** **Step 4:** green + clippy + fmt. **Step 5: Commit** `Add the scenes model, store and evaluator`.

---

### Task 2: Engine in the broker, sensors, apply path

**Files:**
- Create: `apps/desktop/src-tauri/src/scenes/engine.rs`, `src/sensors_macos.rs`
- Modify: `src/broker.rs`, `src/lib.rs`, `src/commands.rs`
- Test: `tests/scenes_engine.rs` (fake transport + scripted inputs)

**Interfaces:**
- Produces: `pub struct Engine { scenes: Vec<Scene>, active: Option<String> }` with `fn tick(&mut self, inputs: &Inputs) -> Option<Actions>` (returns actions only when the active id changes) and `fn apply_manual(&mut self, id: &str) -> Option<Actions>`; `pub fn apply_actions(session: &mut Session, a: &Actions) -> Vec<Result<(), SessionError>>` (one setter call per `Some` field; a failure does not stop the rest); `BrokerCmd::{ApplyScene(String), SetScenes(Vec<Scene>)}`; Tauri commands `get_scenes() -> Vec<Scene>`, `set_scenes(scenes: Vec<Scene>) -> Result<(), String>` (validates ranges, saves, forwards), `apply_scene(id: String) -> Result<(), String>`; event `"scenes"` with `{ scenes, active }` after every change; `sensors_macos.rs`: `pub fn frontmost_app_name() -> Option<String>` (NSWorkspace sharedWorkspace frontmostApplication localizedName), `pub fn mic_in_use() -> bool` (`kAudioDevicePropertyDeviceIsRunningSomewhere` on the default input device); other platforms return `None`/`false`. The broker builds `Inputs` every 2 s (connected from the session, local time, sensors) and calls `tick`.

- [ ] **Step 1: Failing tests** — `unchanged_active_scene_sends_nothing` (two ticks with the same inputs → second returns None and the fake receives no new frames), `partial_refusal_keeps_rest` (session with caps lacking 0x53: a scene with `ult` and `nc` → nc applied/confirmed, ult `Unavailable`, no panic), `manual_apply_sends_actions`, `connect_then_scene_applies_once`.
- [ ] **Steps 2–4:** red → implement → green (full workspace checks). **Step 5: Commit** `Run scenes from the broker with desktop sensors`.

---

### Task 3: Scenes screen and popover Mode row

**Files:**
- Create: `apps/desktop/src/views/Scenes.svelte` (replace the placeholder), `src/lib/scenes.ts` (types mirror + store fed by `get_scenes` and the `"scenes"` event), `src/lib/controls/SceneCard.svelte`
- Modify: `src/views/Popover.svelte` (MODE row), `src/views/Noise.svelte` ("Automatic" block shows the two most relevant scenes and "Edit in Scenes"), `src/lib/ipc.ts`
- Test: `src/lib/scenes.test.ts` (validation helpers), `SceneCard.test.ts`

- [ ] Build the Scenes screen exactly as the mockup: left list (name, trigger sentence, action chips with ULT chips in orange, ACTIVE/OFF label, first-match note, "New scene"), right editor (When row with a trigger picker: Manual / Headphones connect / Time range with two HH:MM fields / App in front with a text field / Microphone in use; Noise boxes NC·AMB·OFF·Keep; Ambient level ticks; Focus on voice toggle; ULT boxes OFF·1·2·Keep; Equalizer boxes with the preset names + Keep; Volume ticks 0..30 (step 1, shown as a tick bar of 30) + Keep; DSEE auto-off toggle + Keep; Hotkey recorder (press keys, shows chips, Clear); Delete / Save scene). Trigger sentences: "Headphones connect", "Between 22:00 and 07:00", "Zoom is in front", "Microphone is in use", "By hand only".
- [ ] Popover MODE row per Global Constraints. Noise "Automatic" block.
- [ ] `pnpm test`, `pnpm check`, hex grep, fake-device walkthrough (create, edit, reorder by drag or up/down buttons, apply by hand, delete). **Commit** `Add the Scenes screen and popover mode row`.

---

### Task 4: Hotkeys

**Files:**
- Modify: `src-tauri/Cargo.toml`, `src/lib.rs`, `src/scenes/engine.rs` (or new `src/hotkeys.rs`), `capabilities/default.json`, `src/views/Settings.svelte` (Hotkeys row becomes available: lists scene hotkeys)
- Test: `tests/hotkeys.rs` (pure function `plan_registrations(scenes) -> Vec<(String, String)>` and conflict mapping)

- [ ] Register one global shortcut per enabled scene with a hotkey; on trigger → `apply_scene`; re-register on `set_scenes`; conflicts/failures produce `{ id, error }` entries emitted with `"scenes"` so the editor's Hotkey row shows refused + reason (`hotkey_conflict_is_refused`). **Commit** `Register global hotkeys for scenes`.

---

### Task 5: Docs and validation

- [ ] README: Modes section (what triggers exist on macOS, how first-match works, hotkeys). `docs/hardware-validation.md`: a Phase 2b section to fill on the owner's next hands-on session (apply by hand, connect trigger, mic trigger with a FaceTime call, hotkey). Commit, push, merge per the owner's standing instruction.
