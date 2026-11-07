import { describe, expect, it, vi } from "vitest";
import { render } from "@testing-library/svelte";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(async () => null) }));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(async () => () => {}),
  emitTo: vi.fn(async () => {}),
}));

import scaleSrc from "../lib/controls/FaderScale.svelte?raw";
import fadersSrc from "../lib/controls/Faders.svelte?raw";
import { emptyState, type DeviceState } from "../lib/state";
import Sound from "./Sound.svelte";

const withEq = (): DeviceState => ({
  ...emptyState(),
  connected: true,
  eq_ult: { value: { preset: 0x10, ult: "Off", bands: [10, 12, 8, 10, 10, 10] }, status: "Confirmed" },
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
    const { container, getAllByRole } = render(Sound, { st: withEq() });
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
});
