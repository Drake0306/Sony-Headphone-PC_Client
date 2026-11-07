// Typed wrappers around the Tauri commands and the app events ("state",
// "settings", "scenes", "frames", "recording").

import { invoke } from "@tauri-apps/api/core";
import { emitTo, listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { CapabilityRow, FrameEntry, RecordingEvent } from "./advanced";
import type { Scene, ScenesEvent } from "./scenes";
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
 * Queues raw bytes on table "t1" or "t2"; rejects with the reason (a non-GET
 * without `confirm === "SEND"`, a deny-listed payload, malformed hex, or the
 * Advanced panel switched off). The reply shows in the frame log.
 */
export const sendRaw = (table: "t1" | "t2", hex: string, confirm: string) =>
  invoke<null>("send_raw", { table, hex, confirm });

export const onFrames = (handler: (frames: FrameEntry[]) => void): Promise<UnlistenFn> =>
  listen<FrameEntry[]>("frames", (event) => handler(event.payload));

export const onRecording = (handler: (e: RecordingEvent) => void): Promise<UnlistenFn> =>
  listen<RecordingEvent>("recording", (event) => handler(event.payload));

/** Where the popover leaves the tab for a main window it has just created (not listening yet). */
export const OPEN_TAB_KEY = "hairline.openTab";

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
