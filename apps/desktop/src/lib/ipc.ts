// Typed wrappers around the Tauri commands and the app events ("state",
// "settings", "scenes", "frames", "recording", "raw", "battery", "update").

import { invoke } from "@tauri-apps/api/core";
import { emitTo, listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { BatteryEstimate } from "./battery";
import type { CapabilityRow, FrameEntry, ProbeEvent, RawEvent, RecordingEvent } from "./advanced";
import type { Scene, ScenesEvent } from "./scenes";
import type { UpdateInfo, UpdatePhase } from "./update";
import type { AppSettings, DeviceState, RuntimeInfo } from "./state";

export type NcModeArg = "off" | "nc" | "ambient";

export const getRuntimeInfo = () => invoke<RuntimeInfo>("runtime_info");

export const getState = () => invoke<DeviceState>("get_state");

export const onState = (handler: (state: DeviceState) => void): Promise<UnlistenFn> =>
  listen<DeviceState>("state", (event) => handler(event.payload));

export const setNc = (mode: NcModeArg, level: number, voiceFocus: boolean) =>
  invoke<void>("set_nc", { mode, level, voiceFocus });

export const setUlt = (level: 0 | 1 | 2) => invoke<void>("set_ult", { level });

export const setMultipoint = (on: boolean) => invoke<void>("set_multipoint", { on });

/** One of the preset ids the device offers (0x00, 0x10..=0x17, 0xA0..=0xA2). */
export const setEqPreset = (preset: number) => invoke<void>("set_eq_preset", { preset });

/** Six band levels, each 0..=20 (10 is flat). */
export const setEqBands = (bands: number[]) => invoke<void>("set_eq_bands", { bands });

/** The Advanced equalizer's ten-band curve, levels −10..=10 (31 Hz .. 16 kHz). */
export const getEqCurve = () => invoke<{ bands: number[] }>("get_eq_curve");

/** Saves the ten-band curve in the app (never sent to the headphones); rejects when invalid. */
export const setEqCurve = (bands: number[]) => invoke<void>("set_eq_curve", { bands });

/** The six device levels (−10..=10, wire order) fitted to the saved curve. */
export const getBestFitPreview = () => invoke<number[]>("best_fit_preview");

/**
 * Saves these ten bands, then writes their best fit into `preset` (Manual,
 * Custom 1 or Custom 2) in one EQ write that keeps ULT; returns the six
 * levels. Rejects when the headphones' EQ is not six bands of 21 steps, or the
 * preset is not one of those three.
 */
export const sendBestFit = (bands: number[], preset: number) =>
  invoke<number[]>("send_best_fit", { bands, preset });

/**
 * Writes a pasted Sound share code: six raw bands (0..=20) and ULT 0..=2 into
 * `preset` (Manual, Custom 1 or Custom 2) in one EQ write. Only called after
 * the confirmation dialog.
 */
export const setEqPresetUltBands = (preset: number, ult: 0 | 1 | 2, bands: number[]) =>
  invoke<void>("set_eq_preset_ult_bands", { preset, ult, bands });

/** Volume 0..=30. */
export const setVolume = (volume: number) => invoke<void>("set_volume", { volume });

/** True lets DSEE Extreme switch itself off automatically. */
export const setDseeAutoOff = (auto: boolean) => invoke<void>("set_dsee_auto_off", { auto });

/** 0 or 1 (sound quality / stable connection). */
export const setConnectionMode = (value: number) => invoke<void>("set_connection_mode", { value });

/** 0x00 5 min, 0x04 15 min, 0x01 30 min, 0x02 60 min, 0x03 180 min, 0x10 off-ear, 0x11 disabled. */
export const setAutoPowerOff = (value: number) => invoke<void>("set_auto_power_off", { value });

export const setWearPause = (on: boolean) => invoke<void>("set_wear_pause", { on });

/** 0x30 phone, 0x31 Google, 0x32 Alexa, 0x34 Sony, 0xFF none. */
export const setVoiceAssistant = (value: number) => invoke<void>("set_voice_assistant", { value });

export const setQuickAccess = (functions: number[]) => invoke<void>("set_quick_access", { functions });

/** -2..=2. */
export const setVoiceGuideVolume = (value: number) => invoke<void>("set_voice_guide_volume", { value });

export const setTouchPanel = (on: boolean) => invoke<void>("set_touch_panel", { on });

export const setSidetone = (on: boolean) => invoke<void>("set_sidetone", { on });

export const connect = () => invoke<void>("connect");

export const openMainWindow = () => invoke<void>("open_main_window");

export const getSettings = () => invoke<AppSettings>("get_settings");

export const setSettings = (s: AppSettings) => invoke<void>("set_settings", { s });

export const onSettings = (handler: (s: AppSettings) => void): Promise<UnlistenFn> =>
  listen<AppSettings>("settings", (event) => handler(event.payload));

/** The managed scene list; the broker then re-emits "scenes" with the active id. */
export const getScenes = () => invoke<Scene[]>("get_scenes");

/** Replaces the whole list; rejects with a human message when a scene is invalid. */
export const setScenes = (scenes: Scene[]) => invoke<void>("set_scenes", { scenes });

/** Applies a scene now (or on the next connect while it stays active). */
export const applySceneCmd = (id: string) => invoke<void>("apply_scene", { id });

export const onScenes = (handler: (e: ScenesEvent) => void): Promise<UnlistenFn> =>
  listen<ScenesEvent>("scenes", (event) => handler(event.payload));

/** Every declared or proven support function, T1 then T2 (Advanced screen). */
export const capabilityList = () => invoke<CapabilityRow[]>("capability_list");

/** Streams "frames" snapshots while on; `true` emits one at once. */
export const watchFrames = (on: boolean) => invoke<null>("watch_frames", { on });

/** Starts or stops the frame recording; the file being written, or null once stopped. */
export const recordFrames = (on: boolean) => invoke<string | null>("record_frames", { on });

/**
 * Hands raw bytes on table "t1" or "t2" to the broker; rejects with the
 * reason (a non-GET without `confirm === "SEND"`, a deny-listed payload,
 * malformed hex, not connected, or the Advanced panel switched off). The
 * "raw" event then says whether the session queued it; the reply shows in
 * the frame log.
 */
export const sendRaw = (table: "t1" | "t2", hex: string, confirm: string) =>
  invoke<null>("send_raw", { table, hex, confirm });

/** What the broker did with a raw request: queued, or refused and why. */
export const onRaw = (handler: (e: RawEvent) => void): Promise<UnlistenFn> =>
  listen<RawEvent>("raw", (event) => handler(event.payload));

export const onFrames = (handler: (frames: FrameEntry[]) => void): Promise<UnlistenFn> =>
  listen<FrameEntry[]>("frames", (event) => handler(event.payload));

/**
 * Starts the opcode probe; rejects unless the Advanced panel is on, the
 * headphones are connected and `confirm === "RUN"`. Progress and results
 * arrive as "probe" events.
 */
export const probeStart = (confirm: string) => invoke<null>("probe_start", { confirm });

/** Stops the probe; the results so far are kept and emitted. */
export const probeStop = () => invoke<null>("probe_stop");

export const onProbe = (handler: (e: ProbeEvent) => void): Promise<UnlistenFn> =>
  listen<ProbeEvent>("probe", (event) => handler(event.payload));

/** The battery history's time-left estimate now; later ones arrive as "battery" events. */
export const batteryEstimate = () => invoke<BatteryEstimate>("battery_estimate");

/** Deletes the battery history; a "battery" event with the empty estimate follows. */
export const clearBatteryHistory = () => invoke<null>("clear_battery_history");

export const onBattery = (handler: (e: BatteryEstimate) => void): Promise<UnlistenFn> =>
  listen<BatteryEstimate>("battery", (event) => handler(event.payload));

export const onRecording = (handler: (e: RecordingEvent) => void): Promise<UnlistenFn> =>
  listen<RecordingEvent>("recording", (event) => handler(event.payload));

/** The running app's version. */
export const currentVersion = () => invoke<string>("current_version");

/** The release the app last found, the version put off with Later and the install's last phase (no network). */
export const updateStatus = () =>
  invoke<{ available: UpdateInfo | null; dismissed: string | null; phase: UpdatePhase | null; installing: boolean }>(
    "update_status",
  );

/** Asks GitHub now; an "update" event follows (available or up to date). Rejects with the reason. */
export const checkUpdate = () => invoke<UpdateInfo | null>("check_update");

/** Downloads, verifies, installs and relaunches; "update" events report each step. Only from Update now. */
export const installUpdate = () => invoke<null>("install_update");

/** Later: hides this version's notice in every window until the next launch. */
export const dismissUpdate = (version: string) => invoke<null>("dismiss_update", { version });

export const onUpdate = (handler: (e: UpdatePhase) => void): Promise<UnlistenFn> =>
  listen<UpdatePhase>("update", (event) => handler(event.payload));

export const onUpdateDismissed = (handler: (version: string | null) => void): Promise<UnlistenFn> =>
  listen<string | null>("update-dismissed", (event) => handler(event.payload));

/** Where the popover leaves the tab for a main window it has just created (not listening yet). */
export const OPEN_TAB_KEY = "shdc.openTab";

/** Opens the main window on `tab`: the "navigate" event for an open window, localStorage for a new one. */
export async function openMainWindowOn(tab: string) {
  try {
    localStorage.setItem(OPEN_TAB_KEY, tab);
  } catch {
    // Storage may be blocked; an open window still gets the event.
  }
  await openMainWindow();
  await emitTo("main", "navigate", tab);
}

export const onNavigate = (handler: (tab: string) => void): Promise<UnlistenFn> =>
  listen<string>("navigate", (event) => handler(event.payload));

/**
 * Calls `handler` with the current state and every later one; the event wins
 * if it arrives before `get_state` answers. Returns the cleanup.
 */
export function watchState(handler: (state: DeviceState) => void): () => void {
  let heard = false;
  const unlisten = onState((s) => {
    heard = true;
    handler(s);
  });
  getState()
    .then((s) => heard || handler(s))
    .catch((e) => console.error(e));
  return () => {
    unlisten.then((stop) => stop()).catch(() => {});
  };
}
