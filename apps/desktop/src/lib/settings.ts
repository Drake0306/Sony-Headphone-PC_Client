// The app settings as a Svelte store, fed by `get_settings` and the
// "settings" event (emitted to every window after `set_settings`).

import { readable } from "svelte/store";
import { getSettings, onSettings } from "./ipc";
import type { AppSettings } from "./state";

/** `null` until the first answer arrives. */
export const settings = readable<AppSettings | null>(null, (set) => {
  let heard = false;
  const unlisten = onSettings((s) => {
    heard = true;
    set(s);
  });
  getSettings()
    .then((s) => heard || set(s))
    .catch((e) => console.error(e));
  return () => {
    unlisten.then((stop) => stop()).catch(() => {});
  };
});
