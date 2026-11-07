// Battery history: the time-left estimate from `battery.jsonl`, fed by the
// `battery_estimate` command and the "battery" event.

import { readable } from "svelte/store";
import { batteryEstimate as fetchEstimate, onBattery } from "./ipc";

/** The `"battery"` event payload. */
export interface BatteryEstimate {
  /** Rounded to 10 min; null when there is not enough data. */
  minutes_left: number | null;
  /** Samples in the history file. */
  samples: number;
  /** The oldest sample, unix seconds. */
  since: number | null;
}

export const EMPTY_ESTIMATE: BatteryEstimate = { minutes_left: null, samples: 0, since: null };

/** "about 5 h 20 m left", "about 40 m left"; empty without an estimate. */
export function timeLeft(minutes: number | null): string {
  if (minutes === null || minutes <= 0) return "";
  const h = Math.floor(minutes / 60);
  const m = minutes % 60;
  const parts = [h > 0 ? `${h} h` : "", m > 0 || h === 0 ? `${m} m` : ""].filter(Boolean);
  return `about ${parts.join(" ")} left`;
}

/** The Settings note: "N samples since <local date>", or "No samples yet". */
export function historyNote(e: BatteryEstimate): string {
  if (e.samples === 0 || e.since === null) return "No samples yet";
  const date = new Date(e.since * 1000).toLocaleDateString();
  return `${e.samples} sample${e.samples === 1 ? "" : "s"} since ${date}`;
}

/** The latest estimate; empty until the first answer. */
export const battery = readable<BatteryEstimate>(EMPTY_ESTIMATE, (set) => {
  let heard = false;
  const unlisten = onBattery((e) => {
    heard = true;
    set(e);
  });
  fetchEstimate()
    .then((e) => heard || set(e ?? EMPTY_ESTIMATE))
    .catch((e) => console.error(e));
  return () => {
    unlisten.then((stop) => stop()).catch(() => {});
  };
});
