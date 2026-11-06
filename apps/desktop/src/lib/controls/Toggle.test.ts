import { describe, expect, it, vi } from "vitest";
import { fireEvent, render } from "@testing-library/svelte";
import Toggle from "./Toggle.svelte";

describe("Toggle", () => {
  it("toggle_click_reports_the_flipped_value", async () => {
    const onpick = vi.fn();
    const { getByRole } = render(Toggle, { on: true, status: "Confirmed", onpick });
    const sw = getByRole("switch");
    expect(sw).toHaveClass("on");
    expect(sw).toHaveAttribute("aria-checked", "true");
    await fireEvent.click(sw);
    expect(onpick).toHaveBeenCalledWith(false);
  });

  it("toggle_refused_shows_reason", () => {
    const { getByRole } = render(Toggle, { on: false, status: { Refused: { reason: "busy" } }, onpick: () => {} });
    expect(getByRole("switch")).toHaveClass("refused");
    expect(getByRole("switch")).toHaveAttribute("title", "busy");
  });

  it("toggle_unavailable_is_inert", async () => {
    const onpick = vi.fn();
    const { getByRole } = render(Toggle, { on: null, status: "Unavailable", onpick });
    const sw = getByRole("switch");
    expect(sw).toHaveClass("unavailable");
    expect(sw).toHaveAttribute("aria-disabled", "true");
    await fireEvent.click(sw);
    expect(onpick).not.toHaveBeenCalled();
  });
});
