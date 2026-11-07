import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render } from "@testing-library/svelte";
import Fader from "./Fader.svelte";

// jsdom has no layout: the rail reports a 210 px tall box (10 px knob, so each
// of the 20 steps is 10 px; raw value v sits at clientY = 205 - 10 v).
const RECT = { top: 0, bottom: 210, height: 210, left: 0, right: 24, width: 24, x: 0, y: 0 };
const yFor = (v: number) => 205 - 10 * v;

beforeEach(() => {
  vi.spyOn(Element.prototype, "getBoundingClientRect").mockReturnValue({ ...RECT, toJSON: () => RECT } as DOMRect);
});

afterEach(() => {
  vi.restoreAllMocks();
  vi.useRealTimers();
});

const props = (over: Record<string, unknown> = {}) => ({
  value: 10,
  label: "1 kHz",
  status: "Confirmed" as const,
  onchange: vi.fn(),
  oncommit: vi.fn(),
  ...over,
});

describe("Fader", () => {
  it("fader_snaps_to_segments_and_clamps", async () => {
    const p = props();
    const { container } = render(Fader, p);
    const rail = container.querySelector(".rail")!;
    // Every pointer position, including far outside the rail, lands on one of
    // the 21 segments: an integer in 0..=20 (shown as −10..+10).
    for (let y = -400; y <= 600; y += 3.7) {
      await fireEvent.pointerDown(rail, { clientY: y, pointerId: 1 });
      await fireEvent.pointerUp(rail, { clientY: y, pointerId: 1 });
    }
    const seen = [...p.onchange.mock.calls, ...p.oncommit.mock.calls].map(([v]) => v as number);
    expect(seen.length).toBeGreaterThan(0);
    for (const v of seen) {
      expect(Number.isInteger(v)).toBe(true);
      expect(v).toBeGreaterThanOrEqual(0);
      expect(v).toBeLessThanOrEqual(20);
    }
    p.onchange.mockClear();
    // Nearest segment: 13.4 steps up snaps to 13, 13.6 to 14; beyond the ends clamps.
    for (const [y, want] of [
      [yFor(13.4), 13],
      [yFor(13.6), 14],
      [yFor(-3), 0],
      [yFor(25), 20],
    ] as const) {
      await fireEvent.pointerDown(rail, { clientY: y, pointerId: 1 });
      await fireEvent.pointerUp(rail, { clientY: y, pointerId: 1 });
      expect(p.onchange).toHaveBeenLastCalledWith(want);
    }
    // A drag commits the snapped value on release.
    await fireEvent.pointerDown(rail, { clientY: yFor(10), pointerId: 1 });
    await fireEvent.pointerMove(rail, { clientY: yFor(16.2), pointerId: 1 });
    await fireEvent.pointerUp(rail, { clientY: yFor(16.2), pointerId: 1 });
    expect(p.oncommit).toHaveBeenLastCalledWith(16);
  });

  it("fader_keyboard_steps", async () => {
    vi.useFakeTimers();
    const cases: [number, string, boolean, number][] = [
      [10, "ArrowUp", false, 11],
      [10, "ArrowDown", false, 9],
      [10, "ArrowUp", true, 15],
      [10, "ArrowDown", true, 5],
      [18, "ArrowUp", true, 20],
      [2, "ArrowDown", true, 0],
      [7, "Home", false, 0],
      [7, "End", false, 20],
    ];
    for (const [value, key, shiftKey, want] of cases) {
      const p = props({ value });
      const { getByRole, unmount } = render(Fader, p);
      await fireEvent.keyDown(getByRole("slider"), { key, shiftKey });
      expect(p.onchange).toHaveBeenLastCalledWith(want);
      expect(p.oncommit).not.toHaveBeenCalled();
      vi.advanceTimersByTime(150);
      expect(p.oncommit).toHaveBeenCalledTimes(1);
      expect(p.oncommit).toHaveBeenLastCalledWith(want);
      unmount();
    }
    // Inert while pending: no keys, no pointer.
    const p = props({ status: { Pending: { since_ms: 0 } } });
    const { getByRole } = render(Fader, p);
    await fireEvent.keyDown(getByRole("slider"), { key: "ArrowUp" });
    vi.advanceTimersByTime(150);
    expect(p.onchange).not.toHaveBeenCalled();
    expect(p.oncommit).not.toHaveBeenCalled();
  });

  it("fader_flushes_a_key_step_on_unmount", async () => {
    vi.useFakeTimers();
    const p = props({ value: 10 });
    const { getByRole, unmount } = render(Fader, p);
    await fireEvent.keyDown(getByRole("slider"), { key: "ArrowUp" });
    vi.advanceTimersByTime(50);
    expect(p.oncommit).not.toHaveBeenCalled();
    unmount();
    expect(p.oncommit).toHaveBeenCalledTimes(1);
    expect(p.oncommit).toHaveBeenLastCalledWith(11);
    // Nothing more fires once the timer would have run.
    vi.advanceTimersByTime(500);
    expect(p.oncommit).toHaveBeenCalledTimes(1);
  });

  it("fader_renders_21_segments_and_zero_line", async () => {
    const { container, getByRole, rerender } = render(Fader, props({ value: 15 }));
    const segs = container.querySelectorAll(".seg");
    expect(segs).toHaveLength(21);
    const zero = container.querySelectorAll(".seg.zero");
    expect(zero).toHaveLength(1);
    expect(segs[10]).toBe(zero[0]);
    // Fill runs from zero up to the knob; the value in Doto sits above the label.
    // Above zero the fill starts at the zero step (10 of 20).
    const bottom = (el: Element) => (el as HTMLElement).style.bottom;
    expect(bottom(container.querySelector(".fill")!)).toBe(bottom(segs[10]));
    expect(container.querySelector(".fader .val")).toHaveTextContent("+5");
    expect(container.querySelector(".fader .val")).toHaveClass("dot");
    const foot = [...container.querySelector(".fader .foot")!.children].map((e) => e.className.split(" ")[0]);
    expect(foot).toEqual(["val", "name"]);
    const slider = getByRole("slider");
    expect(slider).toHaveClass("knob");
    expect(slider).toHaveAttribute("aria-valuemin", "0");
    expect(slider).toHaveAttribute("aria-valuemax", "20");
    expect(slider).toHaveAttribute("aria-valuenow", "15");
    await rerender({ value: 6 });
    // Below zero it starts at the knob (step 6 of 20).
    expect(bottom(container.querySelector(".fill")!)).toBe(bottom(container.querySelectorAll(".seg")[6]));
    expect(bottom(segs[6])).not.toBe(bottom(segs[10]));
    // A real minus sign, not a hyphen.
    expect(container.querySelector(".fader .val")!.textContent).toBe("−4");
    await rerender({ value: 10 });
    expect(container.querySelector(".fill")).toBeNull();
    expect(container.querySelector(".fader .val")!.textContent).toBe("0");
    await rerender({ value: null });
    expect(container.querySelector(".knob")).toBeNull();
    expect(container.querySelector(".fader .val")!.textContent).toBe("--");
  });
});
