// Scenes (named modes): a TypeScript mirror of `Scene`
// (apps/desktop/src-tauri/src/scenes/model.rs) as `scenes.json` stores it,
// a store fed by `get_scenes` and the "scenes" event, and the pure helpers
// the Scenes screen, the Noise screen and the popover share.

import { writable, type Readable } from "svelte/store";
import { applySceneCmd, getScenes, onScenes, setScenes } from "./ipc";
import { presetName, type NcMode } from "./state";

export type Trigger =
  | { kind: "manual" }
  | { kind: "headphones_connected" }
  | { kind: "time_range"; from: string; to: string }
  | { kind: "frontmost_app"; name: string }
  | { kind: "mic_in_use" };

export type TriggerKind = Trigger["kind"];

/** What a scene changes; `null` keeps the current setting. */
export interface Actions {
  nc: NcMode | null;
  /** 1..=20 */
  ambient_level: number | null;
  voice_focus: boolean | null;
  /** 0 off, 1, 2 */
  ult: number | null;
  eq_preset: number | null;
  /** 0..=30 */
  volume: number | null;
  dsee_auto_off: boolean | null;
}

export interface Scene {
  id: string;
  name: string;
  enabled: boolean;
  trigger: Trigger;
  actions: Actions;
  /** Tauri accelerator string, e.g. `Alt+Super+1`. */
  hotkey: string | null;
}

/** The "scenes" event payload. */
export interface ScenesEvent {
  scenes: Scene[];
  active: string | null;
  /** Refused or failed hotkeys by scene id (`__window` for the window shortcut); empty when none. */
  hotkey_errors?: Record<string, string>;
}

export interface ScenesView {
  scenes: Scene[];
  active: string | null;
  hotkey_errors?: Record<string, string>;
}

/** Preset ids the backend accepts (commands.rs `EQ_PRESETS`). */
export const EQ_PRESETS = [0x00, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0xa0, 0xa1, 0xa2];
export const VOLUME_MAX = 30;

/**
 * The editor's EQ preset boxes: the presets the device reports (those the
 * backend accepts) or `EQ_PRESETS` when it reports none, plus the scene's
 * saved preset when the list lacks it.
 */
export function eqPresetChoices(device: number[] | null | undefined, saved: number | null): number[] {
  const known = (device ?? []).filter((id) => EQ_PRESETS.includes(id));
  const list = known.length > 0 ? known : EQ_PRESETS.slice();
  return saved !== null && !list.includes(saved) ? [...list, saved] : list;
}
export const AMBIENT_MAX = 20;

// Counts "scenes" events, so a `get_scenes` reply older than an event is dropped.
let events = 0;

const view = writable<ScenesView>({ scenes: [], active: null }, () => {
  const unlisten = onScenes(({ scenes, active, hotkey_errors }) => {
    events += 1;
    view.set({ scenes, active, hotkey_errors: hotkey_errors ?? {} });
  });
  loadScenes().catch((e) => console.error(e));
  return () => {
    unlisten.then((stop) => stop()).catch(() => {});
  };
});

/** The scene list and the active id. `get_scenes` makes the broker re-emit "scenes", which carries `active`. */
export const scenes: Readable<ScenesView> = { subscribe: view.subscribe };

/** Reads the list; the "scenes" event that follows brings the active id. */
export async function loadScenes(): Promise<Scene[]> {
  const seen = events;
  const list = await getScenes();
  if (events === seen) view.update((v) => ({ ...v, scenes: list }));
  return list;
}

/** Validates and saves the whole list. Returns the error message, or null on success. */
export async function saveScenes(list: Scene[]): Promise<string | null> {
  const error = validateScenes(list);
  if (error !== null) return error;
  try {
    await setScenes(list);
    return null;
  } catch (e) {
    return typeof e === "string" ? e : e instanceof Error ? e.message : String(e);
  }
}

export const applyScene = (id: string) => applySceneCmd(id);

export function triggerSentence(t: Trigger): string {
  switch (t.kind) {
    case "headphones_connected":
      return "Headphones connect";
    case "time_range":
      return `Between ${t.from} and ${t.to}`;
    case "frontmost_app":
      return t.name.trim() === "" ? "An app is in front" : `${t.name.trim()} is in front`;
    case "mic_in_use":
      return "Microphone is in use";
    case "manual":
      return "By hand only";
  }
}

export interface Chip {
  label: string;
  /** Drawn orange, like an active ULT box. */
  ult?: boolean;
}

/** The chips a scene card shows, in a fixed order (the mockup's wording); Keep fields show nothing. */
export function actionChips(a: Actions): Chip[] {
  const chips: Chip[] = [];
  const level = a.ambient_level !== null ? { label: `Ambient ${a.ambient_level}` } : null;
  if (a.nc === "NoiseCancelling") chips.push({ label: "NC" });
  if (a.nc === "Off") chips.push({ label: "Off" });
  if (level) chips.push(level);
  else if (a.nc === "Ambient") chips.push({ label: "Ambient" });
  if (a.voice_focus !== null) chips.push({ label: a.voice_focus ? "Voice focus" : "Voice focus off" });
  if (a.ult !== null) chips.push(a.ult === 0 ? { label: "ULT off" } : { label: `ULT ${a.ult}`, ult: true });
  if (a.eq_preset !== null) chips.push({ label: presetName(a.eq_preset) });
  if (a.volume !== null) chips.push({ label: `Volume ${a.volume}` });
  if (a.dsee_auto_off !== null) chips.push({ label: a.dsee_auto_off ? "DSEE auto" : "DSEE off" });
  return chips;
}

/** `parse_hhmm` (scenes/eval.rs): H or HH, a colon, MM; 00:00..23:59. */
export function isHhmm(s: string): boolean {
  const m = /^(\d{1,2}):(\d{2})$/.exec(s.trim());
  return m !== null && Number(m[1]) < 24 && Number(m[2]) < 60;
}

const inRange = (v: number, lo: number, hi: number) => Number.isInteger(v) && v >= lo && v <= hi;
const hex = (v: number) => `0x${v.toString(16).toUpperCase().padStart(2, "0")}`;

/** `check_scenes` (commands.rs) for one scene: the backend's message, or null. */
export function validateScene(s: Scene): string | null {
  const bad = (field: string, why: string) => `scene ${JSON.stringify(s.name)}: ${field} ${why}`;
  if (s.id === WINDOW_ERROR_KEY) return bad("id", `${JSON.stringify(s.id)} is reserved`);
  if (s.name.trim() === "") return bad("name", "is empty");
  const t = s.trigger;
  if (t.kind === "time_range") {
    for (const [field, value] of [
      ["from", t.from],
      ["to", t.to],
    ] as const) {
      if (!isHhmm(value)) return bad(field, `${JSON.stringify(value)} is not HH:MM`);
    }
  }
  if (t.kind === "frontmost_app" && t.name.trim() === "") return bad("app name", "is empty");
  const a = s.actions;
  if (a.ambient_level !== null && !inRange(a.ambient_level, 1, AMBIENT_MAX))
    return bad("ambient_level", `${a.ambient_level} is outside 1..=${AMBIENT_MAX}`);
  if (a.voice_focus !== null && a.nc !== "Ambient") return bad("voice_focus", "needs Noise set to Ambient");
  if (a.ult !== null && !inRange(a.ult, 0, 2)) return bad("ult", `${a.ult} is outside 0..=2`);
  if (a.eq_preset !== null && !EQ_PRESETS.includes(a.eq_preset))
    return bad("eq_preset", `${hex(a.eq_preset)} is not a known preset`);
  if (a.volume !== null && !inRange(a.volume, 0, VOLUME_MAX))
    return bad("volume", `${a.volume} is outside 0..=${VOLUME_MAX}`);
  if (s.hotkey !== null && lacksModifier(s.hotkey))
    return bad("hotkey", `${JSON.stringify(s.hotkey)} needs Ctrl, Alt or Super`);
  return null;
}

/** `check_scenes` for the whole list, in the backend's order (repeated id first). */
export function validateScenes(list: Scene[]): string | null {
  const ids = new Set<string>();
  for (const s of list) {
    if (ids.has(s.id)) return `scene ${JSON.stringify(s.name)}: id ${JSON.stringify(s.id)} is used twice`;
    ids.add(s.id);
    const error = validateScene(s);
    if (error !== null) return error;
  }
  return null;
}

export const keepAll = (): Actions => ({
  nc: null,
  ambient_level: null,
  voice_focus: null,
  ult: null,
  eq_preset: null,
  volume: null,
  dsee_auto_off: null,
});

/** A new scene: off, by hand only, keeps everything, no hotkey. */
export const newScene = (): Scene => ({
  id: crypto.randomUUID(),
  name: "New scene",
  enabled: false,
  trigger: { kind: "manual" },
  actions: keepAll(),
  hotkey: null,
});

/** The list with scene `id` moved one place up (-1) or down (1); the order is the priority. */
export function moveScene(list: Scene[], id: string, dir: -1 | 1): Scene[] {
  const i = list.findIndex((s) => s.id === id);
  const j = i + dir;
  if (i < 0 || j < 0 || j >= list.length) return list.slice();
  const next = list.slice();
  [next[i], next[j]] = [next[j], next[i]];
  return next;
}

export interface KeyPress {
  code: string;
  ctrlKey: boolean;
  altKey: boolean;
  shiftKey: boolean;
  metaKey: boolean;
}

const MODIFIER_CODES = /^(Shift|Control|Alt|Meta|OS)(Left|Right)?$/;
const FUNCTION_KEY = /^F([1-9]|1\d|2[0-4])$/i;
// What global-hotkey reads as Ctrl, Alt or Super (hotkey.rs `parse_hotkey`).
const TYPING_SAFE_MODS = new Set([
  "CTRL",
  "CONTROL",
  "ALT",
  "OPTION",
  "SUPER",
  "CMD",
  "COMMAND",
  "COMMANDORCONTROL",
  "COMMANDORCTRL",
  "CMDORCTRL",
  "CMDORCONTROL",
]);

/** A press of Shift, Ctrl, Alt or Meta alone (or no key at all). */
export const isModifierPress = (e: KeyPress): boolean => e.code === "" || MODIFIER_CODES.test(e.code);

/**
 * hotkeys.rs `lacks_modifier`: none of Ctrl, Alt, Super and not an F1–F24
 * key, so the hotkey would take over ordinary typing.
 */
export function lacksModifier(hotkey: string): boolean {
  const parts = hotkey.split("+").map((p) => p.trim());
  const key = parts[parts.length - 1] ?? "";
  if (hotkey.trim() === "" || FUNCTION_KEY.test(key)) return false;
  return !parts.slice(0, -1).some((m) => TYPING_SAFE_MODS.has(m.toUpperCase()));
}

/**
 * Tauri's accelerator string for a key press: `Ctrl`, `Alt`, `Shift`, `Super`
 * in that order, then the key (`KeyA` → `A`, `Digit1` → `1`, others by their
 * code). A bare modifier press gives null, and so does a key without Ctrl,
 * Alt or Super unless it is F1–F24 (it would hijack typing).
 */
export function accelerator(e: KeyPress): string | null {
  if (isModifierPress(e)) return null;
  if (!e.ctrlKey && !e.altKey && !e.metaKey && !FUNCTION_KEY.test(e.code)) return null;
  const key = e.code.replace(/^Key(?=[A-Z]$)/, "").replace(/^Digit(?=\d$)/, "");
  const mods = [e.ctrlKey && "Ctrl", e.altKey && "Alt", e.shiftKey && "Shift", e.metaKey && "Super"].filter(
    (m): m is string => typeof m === "string",
  );
  return [...mods, key].join("+");
}

const MAC_KEYS: Record<string, string> = { Ctrl: "⌃", Alt: "⌥", Shift: "⇧", Super: "⌘" };

/** One accelerator part as a key chip: the macOS symbols on a Mac, the names elsewhere. */
export const keyLabel = (part: string, mac: boolean): string => (mac ? (MAC_KEYS[part] ?? part) : part);

/** An accelerator as key chips (empty for no hotkey). */
export const hotkeyChips = (accelerator: string | null, mac: boolean): string[] =>
  accelerator ? accelerator.split("+").map((part) => keyLabel(part, mac)) : [];

/** Why a scene's hotkey was not registered, or null. */
export const hotkeyError = (v: ScenesView, id: string): string | null => v.hotkey_errors?.[id] ?? null;

/** The fixed shortcut that opens the main window (hotkeys.rs `WINDOW_SHORTCUT`). */
export const WINDOW_SHORTCUT = "Alt+Super+N";

/** Where `hotkey_errors` holds a failed window shortcut (hotkeys.rs `WINDOW_ERROR_KEY`). */
export const WINDOW_ERROR_KEY = "__window";

/** One Settings Hotkeys line; `error` is why a scene's hotkey was not registered. */
export interface HotkeyLine {
  label: string;
  keys: string[];
  error: string | null;
}

/** Settings' Hotkeys list: the window shortcut, then each enabled scene with a hotkey, each with its refusal, if any. */
export function hotkeyList(list: Scene[], mac: boolean, errors: Record<string, string> | undefined): HotkeyLine[] {
  return [
    { label: "Open window", keys: hotkeyChips(WINDOW_SHORTCUT, mac), error: errors?.[WINDOW_ERROR_KEY] ?? null },
    ...list
      .filter((s) => s.enabled && s.hotkey && s.hotkey.trim() !== "")
      .map((s) => ({ label: s.name, keys: hotkeyChips(s.hotkey, mac), error: errors?.[s.id] ?? null })),
  ];
}

/** Up to two scenes for the Noise screen: the active one, then the first enabled other one. */
export function relevantScenes(v: ScenesView): Scene[] {
  const active = v.scenes.find((s) => s.id === v.active);
  const other = v.scenes.find((s) => s.enabled && s.id !== v.active);
  return [active, other].filter((s): s is Scene => s !== undefined);
}

export const MODE_BOXES_MAX = 4;

/** The popover's MODE boxes: the first four enabled scenes, and whether a "…" box leads to the rest. */
export function modeBoxes(list: Scene[]): { shown: Scene[]; more: boolean } {
  const enabled = list.filter((s) => s.enabled);
  return { shown: enabled.slice(0, MODE_BOXES_MAX), more: enabled.length > MODE_BOXES_MAX };
}
