import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, within } from "@testing-library/svelte";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(async () => null) }));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(async () => () => {}),
  emitTo: vi.fn(async () => {}),
}));

import { invoke } from "@tauri-apps/api/core";
import scaleSrc from "../lib/controls/FaderScale.svelte?raw";
import fadersSrc from "../lib/controls/Faders.svelte?raw";
import { encodeAdvanced, encodeSound } from "../lib/presetCode";
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

  it("header_drops_drag_hint_when_locked", () => {
    const head = (preset: number) => {
      const r = render(Sound, { st: withEq(preset) });
      const text = r.container.querySelector(".eqhead h4")!.textContent;
      r.unmount();
      return text;
    };
    expect(head(0x10)).toBe("Equalizer · Bright");
    expect(head(0x00)).toBe("Equalizer · Off");
    expect(head(0xa0)).toBe("Equalizer · Manual · drag a handle");
  });
});

describe("Sound share codes", () => {
  const DEVICE_WRITES = ["set_eq_preset_ult_bands", "set_eq_preset_bands", "set_eq_bands", "send_best_fit", "set_eq_preset", "set_ult"];
  const writes = () => vi.mocked(invoke).mock.calls.filter(([cmd]) => DEVICE_WRITES.includes(cmd));
  const custom1 = (): DeviceState => {
    const st = withEq(0xa1);
    st.eq_ult = { value: { preset: 0xa1, ult: "Ult1", bands: [10, 12, 8, 10, 10, 20] }, status: "Confirmed" };
    return st;
  };

  it("share_from_sound_encodes_device_bands_ult_and_slot", async () => {
    Object.defineProperty(navigator, "clipboard", { value: { writeText: vi.fn(async () => {}) }, configurable: true });
    vi.mocked(invoke).mockClear();
    const r = render(Sound, { st: custom1() });
    await fireEvent.click(r.getByRole("button", { name: "Share" }));
    const dialog = r.getByRole("dialog");
    expect(within(dialog).getByText("Share this preset")).toBeTruthy();
    const code = encodeSound({ bands: [10, 12, 8, 10, 10, 20], ult: 1, slot: 1 });
    expect(within(dialog).getByText(code)).toBeTruthy();
    expect(dialog.querySelector("svg.qr")).not.toBeNull();
    await fireEvent.click(within(dialog).getByRole("button", { name: "Close" }));
    expect(r.queryByRole("dialog")).toBeNull();
    expect(writes()).toEqual([]);
    r.unmount();
  });

  it("share is unavailable on a fixed preset, Off or disconnected", async () => {
    const otherShape = { ...custom1(), eq_capability: { bands: 10, steps: 21, ult2_additional_step: 0, presets: [] } };
    const pending = { ...custom1(), eq_ult: { ...custom1().eq_ult, status: { Pending: { since_ms: 0 } } } } as DeviceState;
    for (const st of [withEq(0x10), withEq(0x00), { ...custom1(), connected: false }, otherShape, pending]) {
      const r = render(Sound, { st });
      const share = r.getByRole("button", { name: "Share" });
      expect(share.getAttribute("aria-disabled")).toBe("true");
      await fireEvent.click(share);
      expect(r.queryByRole("dialog")).toBeNull();
      r.unmount();
    }
  });

  it("pasted_code_waits_for_confirmation", async () => {
    vi.mocked(invoke).mockClear();
    const r = render(Sound, { st: custom1() });
    await fireEvent.click(r.getByRole("button", { name: "Paste a code" }));
    const bands = [0, 20, 10, 10, 5, 15];
    await fireEvent.input(r.getByRole("textbox"), { target: { value: encodeSound({ bands, ult: 2, slot: 3 }) } });
    await fireEvent.click(within(r.getByRole("dialog", { name: "Paste a code" })).getByRole("button", { name: "Apply" }));
    const confirm = r.getByRole("dialog", { name: "Apply this preset to the headphones?" });
    expect(within(confirm).getByText("SHDC writes these six bands and ULT 2 into Custom 1.")).toBeTruthy();
    await new Promise((done) => setTimeout(done, 50));
    expect(writes()).toEqual([]);
    await fireEvent.click(within(confirm).getByRole("button", { name: "Apply" }));
    await vi.waitFor(() => expect(r.queryByRole("dialog")).toBeNull());
    expect(writes()).toEqual([["set_eq_preset_ult_bands", { preset: 0xa1, ult: 2, bands }]]);
    r.unmount();
  });

  it("pasted_advanced_code_saves_only", async () => {
    vi.mocked(invoke).mockClear();
    const onadvanced = vi.fn();
    const r = render(Sound, { st: withEq(0x10), onadvanced });
    await fireEvent.click(r.getByRole("button", { name: "Paste a code" }));
    const bands = [1, 2, 3, 4, 5, -5, -4, -3, -2, -1];
    await fireEvent.input(r.getByRole("textbox"), { target: { value: encodeAdvanced(bands) } });
    await fireEvent.click(within(r.getByRole("dialog")).getByRole("button", { name: "Apply" }));
    await vi.waitFor(() => expect(onadvanced).toHaveBeenCalled());
    expect(vi.mocked(invoke).mock.calls.filter(([c]) => c === "set_eq_curve")).toEqual([["set_eq_curve", { bands }]]);
    expect(writes()).toEqual([]);
    expect(r.queryByRole("dialog")).toBeNull();
    r.unmount();
  });
});
