// TypeScript mirror of `DeviceState` (crates/mdr-session/src/state.rs), the
// mdr-proto types it holds and the app's `AppSettings`, as serde serialises
// them (externally tagged enums).

export type Status =
  | "Confirmed"
  | { Pending: { since_ms: number } }
  | { Refused: { reason: string } }
  | "Unavailable";

export interface Tracked<T> {
  value: T | null;
  status: Status;
}

export type Table = "T1" | "T2";
export type Charging = "NotCharging" | "Charging" | "Unknown" | "Charged";
export type NcMode = "Off" | "NoiseCancelling" | "Ambient";

export interface NcAsmState {
  mode: NcMode;
  /** Ambient level, 1..=20. */
  level: number;
  voice_focus: boolean;
}

export type UltLevel = "Off" | "Ult1" | "Ult2";

export interface EqUltState {
  preset: number;
  ult: UltLevel;
  bands: number[];
}

export interface EqUltCapability {
  bands: number;
  steps: number;
  ult2_additional_step: number;
  presets: [number, string][];
}

export type Codec = "Unsettled" | "Sbc" | "Aac" | "Ldac" | "Lc3" | { Other: number };

export type GsKind = "Boolean" | "List";

export interface GsCapability {
  setting: number;
  kind: GsKind;
  subject: string;
  summary: string;
  elements: [string, string][];
}

export type GsValue = { Bool: boolean } | { Index: number };

export interface GsSetting {
  capability: GsCapability;
  current: GsValue | null;
}

export interface PairedDevice {
  name: string;
  playing: boolean;
}

export interface DeviceState {
  connected: boolean;
  /** Bluetooth address of the linked headphones, as the transport reported it. */
  address: string | null;
  /** RFCOMM channel of the v2 service, when the transport exposes it. */
  channel: number | null;
  model: string | null;
  firmware: string | null;
  color: number | null;
  capabilities_t1: number[];
  capabilities_t2: number[];
  proven: [Table, number][];
  battery: Tracked<[number, Charging]>;
  nc: Tracked<NcAsmState>;
  eq_ult: Tracked<EqUltState>;
  eq_capability: EqUltCapability | null;
  codec: Tracked<Codec>;
  volume: Tracked<number>;
  multipoint: Tracked<boolean>;
  auto_power_off: Tracked<number>;
  dsee: Tracked<boolean>;
  /** DSEE Extreme upscaling mode: true is Auto (upscales when it helps), false is Off. */
  dsee_auto_off: Tracked<boolean>;
  connection_mode: Tracked<number>;
  wear_pause: Tracked<boolean>;
  voice_assistant: Tracked<number>;
  quick_access: Tracked<number[]>;
  /** Voice guide volume, -2..=2. */
  voice_guide_volume: Tracked<number>;
  /** Last reported `PlaybackStatus` (0x01 play, 0x02 pause, 0x03 stop); read-only, not shown yet. */
  playback: number | null;
  touch_panel: Tracked<boolean>;
  sidetone: Tracked<boolean>;
  gs: GsSetting[];
  devices: PairedDevice[];
}

/** `RuntimeInfo` (apps/desktop/src-tauri/src/commands.rs). */
export interface RuntimeInfo {
  /** True when the scripted fake headphones stand in for the real ones. */
  fake: boolean;
  version: string;
}

/** `AppSettings` (apps/desktop/src-tauri/src/settings.rs). */
export type Theme = "paper" | "ink" | "system";

export interface AppSettings {
  theme: Theme;
  launch_at_login: boolean;
  hide_dock_icon: boolean;
  /** Shows the Advanced tab (frame log, capability list, raw commands). */
  show_advanced: boolean;
}

/** `AppSettings::default()`. */
export const defaultSettings = (): AppSettings => ({
  theme: "paper",
  launch_at_login: false,
  hide_dock_icon: true,
  show_advanced: false,
});

const unavailable = <T>(): Tracked<T> => ({ value: null, status: "Unavailable" });

/** `DeviceState::default()`: what the UI shows before the first state arrives. */
export function emptyState(): DeviceState {
  return {
    connected: false,
    address: null,
    channel: null,
    model: null,
    firmware: null,
    color: null,
    capabilities_t1: [],
    capabilities_t2: [],
    proven: [],
    battery: unavailable(),
    nc: unavailable(),
    eq_ult: unavailable(),
    eq_capability: null,
    codec: unavailable(),
    volume: unavailable(),
    multipoint: unavailable(),
    auto_power_off: unavailable(),
    dsee: unavailable(),
    dsee_auto_off: unavailable(),
    connection_mode: unavailable(),
    wear_pause: unavailable(),
    voice_assistant: unavailable(),
    quick_access: unavailable(),
    voice_guide_volume: unavailable(),
    playback: null,
    touch_panel: unavailable(),
    sidetone: unavailable(),
    gs: [],
    devices: [],
  };
}

export const isPending = (s: Status) => typeof s === "object" && "Pending" in s;
export const refusedReason = (s: Status) => (typeof s === "object" && "Refused" in s ? s.Refused.reason : null);

/** Colourway from the device info colour byte (libmdr names in comments). */
export function colourway(color: number | null): string {
  switch (color) {
    case 0x01:
      return "Black";
    case 0x02:
      return "Off White"; // WHITE
    case 0x08:
      return "Forest Gray"; // GREEN
    default:
      return "";
  }
}

export function codecLabel(codec: Codec | null): string {
  switch (codec) {
    case "Sbc":
      return "SBC";
    case "Aac":
      return "AAC";
    case "Ldac":
      return "LDAC";
    case "Lc3":
      return "LC3";
    default:
      return "";
  }
}

/** Marketing model number for the name the device reports. */
export const modelNumber = (model: string | null): string => (model === "ULT WEAR" ? "WH-ULT900N" : (model ?? ""));

/** EQ preset names by id (mdr-proto `preset_name`); the device sends empty names. */
export function presetName(id: number): string {
  const names: Record<number, string> = {
    0x00: "Off",
    0x10: "Bright",
    0x11: "Excited",
    0x12: "Mellow",
    0x13: "Relaxed",
    0x14: "Vocal",
    0x15: "Treble Boost",
    0x16: "Bass Boost",
    0x17: "Speech",
    0xa0: "Manual",
    0xa1: "Custom 1",
    0xa2: "Custom 2",
  };
  return names[id] ?? "Unknown";
}

/** Auto power-off element values (`27 05`) in the order the Settings boxes show them. */
export const AUTO_POWER_OFF: { label: string; value: number; long: string }[] = [
  { label: "5", value: 0x00, long: "5 min" },
  { label: "15", value: 0x04, long: "15 min" },
  { label: "30", value: 0x01, long: "30 min" },
  { label: "60", value: 0x02, long: "60 min" },
  { label: "180", value: 0x03, long: "180 min" },
  { label: "Removed", value: 0x10, long: "When removed" },
  { label: "Never", value: 0x11, long: "Off" },
];

export const autoOffLabel = (value: number | null): string =>
  AUTO_POWER_OFF.find((o) => o.value === value)?.long ?? "";

/** Whether a host in the Link list is playing through the headphones (the broker's CoreAudio check). */
export const hostPlaying = (devices: PairedDevice[]) => devices.some((d) => d.playing);

/** The Bluetooth facts the transport reported: RFCOMM channel and address, or "--". */
export const bluetoothLabel = (channel: number | null, address: string | null): string =>
  [channel !== null ? `channel ${channel}` : "", address ?? ""].filter(Boolean).join(" · ") || "--";

/** Prefixes `label` with "FAKE" on a `--fake-device` run, so it is never mistaken for the real headphones. */
export const withFake = (label: string, fake: boolean): string =>
  fake ? ["FAKE", label].filter((part) => part !== "").join(" · ") : label;

/** The main window's top-right status label. */
export const statusLabel = (connected: boolean, firmware: string | null, fake: boolean): string =>
  withFake(connected ? ["Connected", firmware ? `FW ${firmware}` : ""].filter(Boolean).join(" · ") : "Not connected", fake);
