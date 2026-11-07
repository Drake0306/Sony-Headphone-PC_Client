import { describe, expect, it, vi } from "vitest";
import { fireEvent, render } from "@testing-library/svelte";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(async () => null) }));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(async () => () => {}),
  emitTo: vi.fn(async () => {}),
}));

import { invoke } from "@tauri-apps/api/core";
import scaleSrc from "../lib/controls/FaderScale.svelte?raw";
import fadersSrc from "../lib/controls/Faders.svelte?raw";
import { emptyState, type DeviceState } from "../lib/state";
import Sound from "./Sound.svelte";

const withEq = (preset = 0x10): DeviceState => ({
  ...emptyState(),
  connected: true,
  eq_ult: { value: { preset, ult: "Off", bands: [10, 12, 8, 10, 10, 10] }, status: "Confirmed" },
  eq_capability: { bands: 6, steps: 21, ult2_additional_step: 0, presets: [] },
});

/** The declarations of the first `selector { ... }` rule in a component's styles. */
const rule = (css: string, selector: string) => {
  const at = css.indexOf(`${selector} {`);
  expect(at, selector).toBeGreaterThanOrEqual(0);
  return css.slice(at, css.indexOf("}", at));
};

describe("Sound equalizer", () => {
  it("sound_faders_show_all_end_labels", () => {
    const { container, getAllByRole } = render(Sound, { st: withEq(0xa0) });
    expect(getAllByRole("slider")).toHaveLength(6);
    expect(container.querySelectorAll(".fader")).toHaveLength(6);
    const ends = container.querySelector(".ends")!;
    expect(ends).not.toBeNull();
    const marks = [...ends.querySelectorAll("b")].map((b) => b.textContent);
    expect(marks).toEqual(["+10", "0", "−10"]);
    // The scale sits outside every fader column and rail, so no narrow box holds it.
    expect(ends.closest(".fader, .rail")).toBeNull();
    // Class snapshot of the ancestors up to the equalizer box.
    const chain: string[] = [];
    for (let el: Element | null = ends; el && !el.classList.contains("eq"); el = el.parentElement) {
      chain.push(el.className.replace(/\s*svelte-\w+/g, "").trim());
    }
    expect(chain).toEqual(["ends", "fader-scale", "faders"]);
    // None of those boxes clips, and the marks never wrap.
    for (const [src, cls] of [
      [scaleSrc, ".ends"],
      [scaleSrc, ".fader-scale"],
      [fadersSrc, ".faders"],
    ]) {
      expect(rule(src, cls)).not.toMatch(/overflow\s*:\s*hidden/);
    }
    expect(scaleSrc).not.toMatch(/overflow\s*:\s*hidden/);
    expect(rule(scaleSrc, ".ends")).toMatch(/white-space:\s*nowrap/);
    expect(rule(scaleSrc, ".fader-scale")).toMatch(/flex:\s*none/);
  });

  const FIXED_NOTE =
    "Sony applies this preset inside the headphones and does not report its band values. Choose Manual, Custom 1 or Custom 2 to shape your own.";
  const bandWrites = () => vi.mocked(invoke).mock.calls.filter(([cmd]) => cmd === "set_eq_bands");
  const settle = () => new Promise((done) => setTimeout(done, 200));

  /** Drags the first fader and steps every fader by key; returns after the commit delay. */
  async function tryEveryFader(container: HTMLElement) {
    const rail = container.querySelector(".rail")!;
    await fireEvent.pointerDown(rail, { button: 0, clientY: 1, pointerId: 1 });
    await fireEvent.pointerMove(rail, { clientY: 0, pointerId: 1 });
    await fireEvent.pointerUp(rail, { pointerId: 1 });
    for (const s of container.querySelectorAll("[role=slider]")) await fireEvent.keyDown(s, { key: "ArrowUp" });
    await settle();
  }

  it("fixed_preset_faders_are_inert", async () => {
    vi.mocked(invoke).mockClear();
    const { container, getByText, getAllByRole } = render(Sound, { st: withEq(0x10) });
    expect(getByText(FIXED_NOTE)).toBeTruthy();
    // The device's values are still drawn, faded and inert.
    const sliders = getAllByRole("slider");
    expect(sliders.map((s) => s.getAttribute("aria-valuenow"))).toEqual(["10", "12", "8", "10", "10", "10"]);
    expect(sliders.every((s) => s.getAttribute("aria-disabled") === "true")).toBe(true);
    expect(container.querySelector(".eq .faders")!.classList.contains("unavailable")).toBe(true);
    await tryEveryFader(container);
    expect(bandWrites()).toHaveLength(0);
    expect(getAllByRole("slider").map((s) => s.getAttribute("aria-valuenow"))).toEqual(["10", "12", "8", "10", "10", "10"]);
    // Reset would write bands into the fixed preset too: unavailable.
    expect(getByText("Reset").getAttribute("aria-disabled")).toBe("true");
  });

  it("off_preset_disables_the_faders", async () => {
    vi.mocked(invoke).mockClear();
    const { container, getByText, getAllByRole, queryByText } = render(Sound, { st: withEq(0x00) });
    expect(getByText("Equalizer off.")).toBeTruthy();
    expect(queryByText(FIXED_NOTE)).toBeNull();
    expect(getAllByRole("slider").every((s) => s.getAttribute("aria-disabled") === "true")).toBe(true);
    await tryEveryFader(container);
    expect(bandWrites()).toHaveLength(0);
  });

  it("manual_and_custom_faders_stay_live", async () => {
    for (const preset of [0xa0, 0xa1, 0xa2]) {
      vi.mocked(invoke).mockClear();
      const { getAllByRole, queryByText, unmount } = render(Sound, { st: withEq(preset) });
      expect(queryByText(FIXED_NOTE)).toBeNull();
      expect(queryByText("Equalizer off.")).toBeNull();
      const first = getAllByRole("slider")[0];
      expect(first.getAttribute("aria-disabled")).toBe("false");
      await fireEvent.keyDown(first, { key: "ArrowUp" });
      await settle();
      expect(bandWrites()).toEqual([["set_eq_bands", { bands: [11, 12, 8, 10, 10, 10] }]]);
      unmount();
    }
  });
});
