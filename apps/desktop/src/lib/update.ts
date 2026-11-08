// In-app updates: the running version, the newer release GitHub offers and
// where an install stands, fed by the "update" events (updater.rs), the
// "update-dismissed" event (Later, from any window) and `update_status` for a
// window that opens after the check. Nothing here is stored on disk, so a
// version put off with Later shows again after a relaunch.

import { derived, get, writable } from "svelte/store";
import { checkUpdate, currentVersion, dismissUpdate, installUpdate, onUpdate, onUpdateDismissed, updateStatus } from "./ipc";

export interface UpdateInfo {
  version: string;
  notes: string;
  /** `YYYY-MM-DD`, when the release says. */
  date: string | null;
}

/** `updater::Phase` as serde sends it. */
export type UpdatePhase =
  | ({ phase: "available" } & UpdateInfo)
  | { phase: "downloading"; downloaded: number; total: number | null }
  | { phase: "installing" }
  | { phase: "restarting" }
  | { phase: "failed"; error: string }
  | { phase: "up_to_date" };

export interface UpdateState {
  /** The running app's version ("" until it arrives). */
  current: string;
  available: UpdateInfo | null;
  /** The last thing that happened; `null` before anything did. */
  phase: UpdatePhase | null;
  /** The version put off with Later, until the next launch. */
  dismissedVersion: string | null;
}

export const initial = (): UpdateState => ({ current: "", available: null, phase: null, dismissedVersion: null });

const busy = (p: UpdatePhase | null) =>
  p?.phase === "downloading" || p?.phase === "installing" || p?.phase === "restarting";

/** The state after an "update" event. A background notice never interrupts an install. */
export function reduce(s: UpdateState, e: UpdatePhase): UpdateState {
  switch (e.phase) {
    case "available": {
      const available = { version: e.version, notes: e.notes, date: e.date };
      return { ...s, available, phase: busy(s.phase) ? s.phase : e };
    }
    case "up_to_date":
      return { ...s, available: null, phase: e };
    default:
      return { ...s, phase: e };
  }
}

/** Whole percent downloaded, or null when the size is unknown or not downloading. */
export function percent(p: UpdatePhase | null): number | null {
  if (p?.phase !== "downloading" || !p.total) return null;
  return Math.min(100, Math.floor((p.downloaded / p.total) * 100));
}

const state = writable<UpdateState>(initial(), (_set, update) => {
  const stops = [
    onUpdate((e) => update((s) => reduce(s, e))),
    onUpdateDismissed((version) => update((s) => ({ ...s, dismissedVersion: version }))),
  ];
  currentVersion()
    .then((current) => update((s) => ({ ...s, current })))
    .catch((e) => console.error(e));
  updateStatus()
    .then(({ available, dismissed }) =>
      update((s) => ({ ...s, available: s.available ?? available, dismissedVersion: s.dismissedVersion ?? dismissed })),
    )
    .catch((e) => console.error(e));
  return () => stops.forEach((p) => p.then((stop) => stop()).catch(() => {}));
});

export const update = { subscribe: state.subscribe };

/** True when there is a release to offer that was not put off with Later. */
export const visible = derived(state, (s) => s.available !== null && s.dismissedVersion !== s.available.version);

/** Later: hides this version in every window until the next launch. */
export async function later() {
  const version = get(state).available?.version;
  if (!version) return;
  state.update((s) => ({ ...s, dismissedVersion: version }));
  await dismissUpdate(version).catch((e) => console.error(e));
}

/** Check now. The answer arrives as an "update" event; a failure also lands here in case it did not. */
export async function checkNow() {
  try {
    await checkUpdate();
  } catch (e) {
    state.update((s) => ({ ...s, phase: { phase: "failed", error: typeof e === "string" ? e : String(e) } }));
  }
}

/** Update now: downloads, installs and relaunches; progress arrives as "update" events. */
export async function installNow() {
  try {
    await installUpdate();
  } catch (e) {
    state.update((s) => ({ ...s, phase: { phase: "failed", error: typeof e === "string" ? e : String(e) } }));
  }
}
