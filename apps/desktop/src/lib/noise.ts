// The noise switch, ambient level and voice focus share one SET (`68 17`):
// every pick sends the whole NC/ASM state. Used by the popover, Overview and
// Noise screens.

import { setNc, type NcModeArg } from "./ipc";
import type { Pending } from "./pending";
import type { NcAsmState, NcMode } from "./state";

export const NC_ARG: Record<NcMode, NcModeArg> = { NoiseCancelling: "nc", Ambient: "ambient", Off: "off" };

/** Anything that records a wish and sends it: a `Pending` or a `Control`. */
type Nc = Pick<Pending<NcAsmState>, "request">;

const clampLevel = (level: number | undefined) => Math.min(20, Math.max(1, level ?? 1));

function request(nc: Nc, next: NcAsmState) {
  nc.request(next, () => setNc(NC_ARG[next.mode], next.level, next.voice_focus));
}

/** Switches mode, keeping the confirmed level and voice focus. */
export function pickNoise(nc: Nc, cur: NcAsmState | null, mode: NcMode) {
  request(nc, { mode, level: clampLevel(cur?.level), voice_focus: cur?.voice_focus ?? false });
}

/** Sets the ambient level (Ambient only). */
export function pickLevel(nc: Nc, cur: NcAsmState | null, level: number) {
  request(nc, { mode: "Ambient", level, voice_focus: cur?.voice_focus ?? false });
}

/** Sets voice focus (Ambient only), keeping the level on screen. */
export function pickVoice(nc: Nc, shown: NcAsmState | null, on: boolean) {
  request(nc, { mode: "Ambient", level: clampLevel(shown?.level), voice_focus: on });
}
