import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render } from "@testing-library/svelte";
import Faders from "./Faders.svelte";

const labels = ["Clear Bass", "400 Hz", "1 kHz", "2.5 kHz", "6.3 kHz", "16 kHz"];
const flat = () => [10, 10, 10, 10, 10, 10];

// jsdom has no layout: every rail reports a 210 px tall box (10 px handle, so
// each of the 20 steps is 10 px; raw value v sits at clientY = 205 - 10 v).
const RECT = { top: 0, bottom: 210, height: 210, left: 0, right: 24, width: 24, x: 0, y: 0 };
const yFor = (v: number) => 205 - 10 * v;

beforeEach(() => {
  vi.spyOn(Element.prototype, "getBoundingClientRect").mockReturnValue({ ...RECT, toJSON: () => RECT } as DOMRect);
});

afterEach(() => {
  vi.restoreAllMocks();
  vi.useRealTimers();
});

describe("Faders", () => {
  it("faders_commit_once_on_release", async () => {
    const oncommit = vi.fn();
    const { container, getAllByRole } = render(Faders, {
      bands: flat(),
      labels,
      status: "Confirmed",
      oncommit,
    });
    const rail = container.querySelectorAll(".rail")[1];
    await fireEvent.pointerDown(rail, { clientY: yFor(10), pointerId: 1 });
    for (const v of [11, 12, 13, 14, 15]) await fireEvent.pointerMove(rail, { clientY: yFor(v), pointerId: 1 });
    // The handle and its label follow the pointer before anything is sent.
    expect(oncommit).not.toHaveBeenCalled();
    expect(getAllByRole("slider")[1]).toHaveAttribute("aria-valuenow", "15");
    expect(container.querySelectorAll(".val")[1]).toHaveTextContent("+5");
    await fireEvent.pointerUp(rail, { clientY: yFor(15), pointerId: 1 });
    expect(oncommit).toHaveBeenCalledTimes(1);
    expect(oncommit).toHaveBeenCalledWith([10, 15, 10, 10, 10, 10]);
  });

  it("faders_keyboard_step_commits_after_delay", async () => {
    vi.useFakeTimers();
    const oncommit = vi.fn();
    const { getAllByRole, container } = render(Faders, {
      bands: flat(),
      labels,
      status: "Confirmed",
      oncommit,
    });
    const handle = getAllByRole("slider")[2];
    await fireEvent.keyDown(handle, { key: "ArrowUp" });
    await fireEvent.keyDown(handle, { key: "ArrowUp" });
    expect(container.querySelectorAll(".val")[2]).toHaveTextContent("+2");
    vi.advanceTimersByTime(149);
    expect(oncommit).not.toHaveBeenCalled();
    vi.advanceTimersByTime(1);
    expect(oncommit).toHaveBeenCalledTimes(1);
    expect(oncommit).toHaveBeenCalledWith([10, 10, 12, 10, 10, 10]);
  });

  it("faders_keyboard_clamps_at_bounds", async () => {
    vi.useFakeTimers();
    const oncommit = vi.fn();
    const { getAllByRole } = render(Faders, {
      bands: [20, 0, 10, 10, 10, 10],
      labels,
      status: "Confirmed",
      oncommit,
    });
    const [top, bottom] = getAllByRole("slider");
    await fireEvent.keyDown(top, { key: "ArrowUp" });
    await fireEvent.keyDown(bottom, { key: "ArrowDown" });
    vi.advanceTimersByTime(150);
    expect(oncommit).not.toHaveBeenCalled();
    expect(top).toHaveAttribute("aria-valuenow", "20");
    expect(bottom).toHaveAttribute("aria-valuenow", "0");
  });

  it("faders_pending_are_inert", async () => {
    const oncommit = vi.fn();
    const { container } = render(Faders, {
      bands: flat(),
      labels,
      status: { Pending: { since_ms: 0 } },
      oncommit,
    });
    expect(container.querySelector(".faders")).toHaveClass("pending");
    const rail = container.querySelectorAll(".rail")[0];
    await fireEvent.pointerDown(rail, { clientY: yFor(18), pointerId: 1 });
    await fireEvent.pointerUp(rail, { clientY: yFor(18), pointerId: 1 });
    expect(oncommit).not.toHaveBeenCalled();
  });

  it("faders_refused_reason_is_the_title", () => {
    const { container } = render(Faders, {
      bands: flat(),
      labels,
      status: { Refused: { reason: "no reply" } },
      oncommit: () => {},
    });
    expect(container.querySelector(".faders")).toHaveAttribute("title", "no reply");
  });
});
