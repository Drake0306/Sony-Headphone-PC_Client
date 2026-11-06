// Typed wrappers around the Tauri commands and the "state" event.

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { DeviceState } from "./state";

export type NcModeArg = "off" | "nc" | "ambient";

export const getState = () => invoke<DeviceState>("get_state");

export const onState = (handler: (state: DeviceState) => void): Promise<UnlistenFn> =>
  listen<DeviceState>("state", (event) => handler(event.payload));

export const setNc = (mode: NcModeArg, level: number, voiceFocus: boolean) =>
  invoke<void>("set_nc", { mode, level, voiceFocus });

export const setUlt = (level: 0 | 1 | 2) => invoke<void>("set_ult", { level });

export const setMultipoint = (on: boolean) => invoke<void>("set_multipoint", { on });

export const connect = () => invoke<void>("connect");

export const openMainWindow = () => invoke<void>("open_main_window");
