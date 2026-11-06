import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { get } from "svelte/store";
import { displayStatus, pending, shownValue } from "./pending";

const PENDING = { Pending: { since_ms: 1 } } as const;

describe("pending", () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it("shows_the_wanted_value_while_pending", () => {
    const p = pending<number>();
    p.request(2, () => Promise.resolve());
    expect(shownValue(get(p), PENDING, 1)).toBe(2);
    p.receive("Confirmed");
    expect(shownValue(get(p), "Confirmed", 2)).toBe(2);
    expect(get(p).want).toBeNull();
  });

  it("holds_a_refusal_for_four_seconds", () => {
    const p = pending<number>();
    p.request(2, () => Promise.resolve());
    p.receive({ Refused: { reason: "no reply" } });
    expect(get(p).refused).toEqual({ value: 2, reason: "no reply" });
    expect(displayStatus(get(p), "Confirmed")).toEqual({ Refused: { reason: "no reply" } });
    vi.advanceTimersByTime(4000);
    expect(get(p).refused).toBeNull();
    expect(displayStatus(get(p), { Refused: { reason: "no reply" } })).toBe("Confirmed");
  });

  it("ignores_refusals_it_did_not_ask_for", () => {
    const p = pending<number>();
    p.receive({ Refused: { reason: "other window" } });
    expect(get(p).refused).toBeNull();
  });

  it("drops_the_wanted_value_when_the_send_fails", async () => {
    const p = pending<number>();
    const err = vi.spyOn(console, "error").mockImplementation(() => {});
    p.request(2, () => Promise.reject(new Error("broker stopped")));
    await vi.runAllTimersAsync();
    expect(get(p).want).toBeNull();
    err.mockRestore();
  });

  it("unavailable_and_pending_win_over_a_held_refusal", () => {
    const p = pending<number>();
    p.request(2, () => Promise.resolve());
    p.receive({ Refused: { reason: "no" } });
    expect(displayStatus(get(p), "Unavailable")).toBe("Unavailable");
    expect(displayStatus(get(p), PENDING)).toEqual(PENDING);
  });
});
