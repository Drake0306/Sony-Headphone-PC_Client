// TypeScript mirror of `DeviceState` (crates/mdr-session/src/state.rs) and the
// mdr-proto types it holds, as serde serialises them (externally tagged enums).

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
  gs: GsSetting[];
  devices: PairedDevice[];
}

const unavailable = <T>(): Tracked<T> => ({ value: null, status: "Unavailable" });

/** `DeviceState::default()`: what the UI shows before the first state arrives. */
export function emptyState(): DeviceState {
  return {
    connected: false,
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
