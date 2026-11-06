// Binds one tracked value to `pending.ts` inside a component: the value and
// status to draw, and `request` to send a change. Call during component init.

import { get as read } from "svelte/store";
import { displayStatus, pending, shownValue, type PendingView } from "./pending";
import type { Status } from "./state";

export interface Control<T> {
  /** The wanted value while pending, else the confirmed one. */
  readonly shown: T | null;
  /** The status to draw (a refusal only while held). */
  readonly status: Status;
  /** The refused request and its reason, while shown. */
  readonly refused: { value: T; reason: string } | null;
  request(value: T, send: () => Promise<unknown>): void;
}

export function control<T>(source: () => { value: T | null; status: Status }): Control<T> {
  const p = pending<T>();
  let view = $state<PendingView<T>>(read(p));
  $effect.pre(() => p.subscribe((v) => (view = v)));
  $effect.pre(() => p.receive(source().status));
  return {
    get shown() {
      const s = source();
      return shownValue(view, s.status, s.value);
    },
    get status() {
      return displayStatus(view, source().status);
    },
    get refused() {
      return view.refused;
    },
    request: (value, send) => p.request(value, send),
  };
}
