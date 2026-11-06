// Optimistic display for one tracked value: what the user just asked for is
// shown (striped) while the device has not answered, and a refusal of that
// request stays outlined with its reason for a few seconds. Shared by the
// popover and the window screens.

import { writable, type Readable } from "svelte/store";
import { isPending, refusedReason, type Status } from "./state";

export const REFUSED_SHOWN_MS = 4000;

export interface PendingView<T> {
  /** The value requested and not yet answered. */
  want: T | null;
  /** The last refused request and the device's reason, while it is shown. */
  refused: { value: T; reason: string } | null;
}

export interface Pending<T> extends Readable<PendingView<T>> {
  /** Records `value` as wanted and runs `send`; a failed send drops the wish. */
  request(value: T, send: () => Promise<unknown>): void;
  /** Feeds the tracked value's new status from a state update. */
  receive(status: Status): void;
}

export function pending<T>(holdMs = REFUSED_SHOWN_MS): Pending<T> {
  let view: PendingView<T> = { want: null, refused: null };
  const store = writable(view);
  const update = (next: Partial<PendingView<T>>) => {
    view = { ...view, ...next };
    store.set(view);
  };

  return {
    subscribe: store.subscribe,
    request(value, send) {
      update({ want: value, refused: null });
      send().catch((e) => {
        if (view.want === value) update({ want: null });
        console.error(e);
      });
    },
    receive(status) {
      const reason = refusedReason(status);
      if (reason !== null && view.want !== null) {
        const refused = { value: view.want, reason };
        update({ refused });
        setTimeout(() => view.refused === refused && update({ refused: null }), holdMs);
      }
      if (!isPending(status) && view.want !== null) update({ want: null });
    },
  };
}

/** The value to draw: the wanted one while pending, else the confirmed one. */
export const shownValue = <T>(view: PendingView<T>, status: Status, confirmed: T | null): T | null =>
  isPending(status) && view.want !== null ? view.want : confirmed;

/**
 * The status to draw. Unavailable and Pending come first; a refusal is shown
 * only while held, so a stale `Refused` from the session reads as Confirmed.
 */
export function displayStatus<T>(view: PendingView<T>, status: Status): Status {
  if (status === "Unavailable" || isPending(status)) return status;
  if (view.refused) return { Refused: { reason: view.refused.reason } };
  return refusedReason(status) !== null ? "Confirmed" : status;
}
