import { describe, expect, it, vi } from "vitest";
import { fireEvent, render } from "@testing-library/svelte";
import ThemePicker from "./ThemePicker.svelte";
import { THEMES } from "../theme";
import type { Status } from "../state";

const PENDING: Status = { Pending: { since_ms: 0 } };
const REFUSED: Status = { Refused: { reason: "Not now" } };

describe("ThemePicker", () => {
  it("shows one swatch per theme, the chosen one checked and the only tab stop", () => {
    const r = render(ThemePicker, { value: "nord", status: "Confirmed", onpick: () => {} });
    const radios = r.getAllByRole("radio");
    expect(radios.length).toBe(THEMES.length);
    const checked = radios.filter((b) => b.getAttribute("aria-checked") === "true");
    expect(checked.map((b) => b.textContent?.trim())).toEqual(["Nord"]);
    expect(radios.filter((b) => b.getAttribute("tabindex") === "0").length).toBe(1);
    expect(checked[0].getAttribute("tabindex")).toBe("0");
  });

  it("picks on click and moves the choice with the arrow keys, wrapping", async () => {
    const onpick = vi.fn();
    const r = render(ThemePicker, { value: "system", status: "Confirmed", onpick });
    await fireEvent.click(r.getByText("Mocha").closest("button")!);
    expect(onpick).toHaveBeenLastCalledWith("mocha");
    const first = r.getAllByRole("radio")[0];
    await fireEvent.keyDown(first, { key: "ArrowRight" });
    expect(onpick).toHaveBeenLastCalledWith(THEMES[1].value);
    await fireEvent.keyDown(first, { key: "ArrowLeft" });
    expect(onpick).toHaveBeenLastCalledWith(THEMES[THEMES.length - 1].value);
  });

  it("ignores clicks and keys while pending", async () => {
    const onpick = vi.fn();
    const r = render(ThemePicker, { value: "paper", status: PENDING, onpick });
    await fireEvent.click(r.getByText("Nord").closest("button")!);
    await fireEvent.keyDown(r.getAllByRole("radio")[1], { key: "ArrowRight" });
    expect(onpick).not.toHaveBeenCalled();
  });

  it("outlines the attempted swatch with the reason as its tooltip when refused", () => {
    const r = render(ThemePicker, { value: "paper", status: REFUSED, attempted: "oled", onpick: () => {} });
    const oled = r.getByText("OLED Black").closest("button")!;
    expect(oled.className).toContain("refused");
    expect(oled.getAttribute("title")).toBe("Not now");
    expect(r.getByText("Paper").closest("button")!.className).not.toContain("refused");
  });

  it("is faded, inert and out of the tab order when unavailable", async () => {
    const onpick = vi.fn();
    const r = render(ThemePicker, { value: null, status: "Unavailable", onpick });
    const radios = r.getAllByRole("radio");
    expect(radios.every((b) => b.getAttribute("tabindex") === "-1")).toBe(true);
    expect(radios.every((b) => b.getAttribute("aria-disabled") === "true")).toBe(true);
    await fireEvent.click(radios[2]);
    expect(onpick).not.toHaveBeenCalled();
  });
});
