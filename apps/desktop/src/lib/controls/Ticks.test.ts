import { describe, expect, it, vi } from "vitest";
import { fireEvent, render } from "@testing-library/svelte";
import Ticks from "./Ticks.svelte";

describe("Ticks", () => {
  it("ticks_click_reports_one_based_level", async () => {
    const onpick = vi.fn();
    const { getAllByRole } = render(Ticks, { count: 20, value: 14, status: "Confirmed", onpick });
    const ticks = getAllByRole("button");
    expect(ticks).toHaveLength(20);
    expect(ticks.filter((t) => t.classList.contains("on"))).toHaveLength(14);
    await fireEvent.click(ticks[0]);
    expect(onpick).toHaveBeenLastCalledWith(1);
    await fireEvent.click(ticks[19]);
    expect(onpick).toHaveBeenLastCalledWith(20);
  });

  it("ticks_disabled_are_inert", async () => {
    const onpick = vi.fn();
    const { container, getAllByRole } = render(Ticks, {
      count: 20,
      value: 5,
      status: "Confirmed",
      disabled: true,
      onpick,
    });
    expect(container.querySelector(".ticks")).toHaveClass("unavailable");
    await fireEvent.click(getAllByRole("button")[3]);
    expect(onpick).not.toHaveBeenCalled();
  });

  it("ticks_pending_are_inert", async () => {
    const onpick = vi.fn();
    const { container, getAllByRole } = render(Ticks, {
      count: 20,
      value: 5,
      status: { Pending: { since_ms: 0 } },
      onpick,
    });
    expect(container.querySelector(".ticks")).toHaveClass("pending");
    await fireEvent.click(getAllByRole("button")[3]);
    expect(onpick).not.toHaveBeenCalled();
  });
});
