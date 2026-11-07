import { beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, within } from "@testing-library/svelte";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(async () => null) }));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(async () => () => {}),
  emitTo: vi.fn(async () => {}),
}));

import { invoke } from "@tauri-apps/api/core";
import { encodeAdvanced, encodeSound } from "../presetCode";
import { emptyState, type DeviceState } from "../state";
import PasteCode from "./PasteCode.svelte";

const DEVICE_WRITES = ["set_eq_preset_bands", "set_eq_bands", "send_best_fit", "set_eq_preset", "set_ult"];
const writes = () => vi.mocked(invoke).mock.calls.filter(([cmd]) => DEVICE_WRITES.includes(cmd));
const calls = (cmd: string) => vi.mocked(invoke).mock.calls.filter(([c]) => c === cmd);

const connected = (preset = 0xa0, presets: number[] = []): DeviceState => ({
  ...emptyState(),
  connected: true,
  eq_ult: { value: { preset, ult: "Off", bands: [10, 10, 10, 10, 10, 10] }, status: "Confirmed" },
  eq_capability: { bands: 6, steps: 21, ult2_additional_step: 0, presets: presets.map((id) => [id, ""]) },
});

const SOUND = { bands: [15, 11, 8, 11, 13, 16], ult: 1 as const, slot: 3 as const };
const values = (c: HTMLElement) => [...c.querySelectorAll(".preview .dot")].map((n) => n.textContent);

async function type(r: { getByRole: (role: "textbox") => HTMLElement }, text: string) {
  await fireEvent.input(r.getByRole("textbox"), { target: { value: text } });
}

describe("Paste a code", () => {
  beforeEach(() => vi.mocked(invoke).mockClear());

  it("shows errors live and keeps Apply off until the code is valid", async () => {
    const r = render(PasteCode, { st: connected(), onclose: () => {} });
    const dialog = r.getByRole("dialog");
    expect(within(dialog).getByText("Paste a code")).toBeTruthy();
    const apply = r.getByRole("button", { name: "Apply" });
    expect(apply.getAttribute("aria-disabled")).toBe("true");
    await type(r, "hello");
    expect(r.getByText("That code is not a Hairline preset.")).toBeTruthy();
    const code = encodeSound(SOUND);
    const typo = code.slice(0, 7) + (code[7] === "1" ? "2" : "1") + code.slice(8);
    await type(r, typo);
    expect(r.getByText("That code has a typo (check failed).")).toBeTruthy();
    expect(apply.getAttribute("aria-disabled")).toBe("true");
    await type(r, code.toLowerCase());
    expect(apply.getAttribute("aria-disabled")).toBe("false");
    expect(values(r.container)).toEqual(["+5", "+1", "−2", "+1", "+3", "+6"]);
    expect(r.getByText("Clear Bass")).toBeTruthy();
    expect(r.getByText(/ULT 1/)).toBeTruthy();
    expect(r.getByText(/Manual/)).toBeTruthy();
    r.unmount();
  });

  it("pasted_code_waits_for_confirmation", async () => {
    const onclose = vi.fn();
    const r = render(PasteCode, { st: connected(0x10), onclose });
    await type(r, encodeSound(SOUND));
    // A fixed preset now, no slot in the code: Custom 1, as Send best fit.
    expect(r.getByText(/Custom 1 \(the headphones will switch to it\)/)).toBeTruthy();
    await fireEvent.click(r.getByRole("button", { name: "Apply" }));
    const dialog = r.getByRole("dialog");
    expect(within(dialog).getByText("Apply this preset to the headphones?")).toBeTruthy();
    expect(
      within(dialog).getByText(
        "Hairline writes these six bands and ULT 1 into Custom 1 (the headphones will switch to it).",
      ),
    ).toBeTruthy();
    expect(writes()).toEqual([]);
    // Cancel goes back to the code; still nothing sent.
    await fireEvent.click(within(dialog).getByRole("button", { name: "Cancel" }));
    expect(writes()).toEqual([]);
    expect(onclose).not.toHaveBeenCalled();
    await fireEvent.click(r.getByRole("button", { name: "Apply" }));
    await fireEvent.click(within(r.getByRole("dialog")).getByRole("button", { name: "Apply" }));
    await vi.waitFor(() => expect(onclose).toHaveBeenCalled());
    expect(writes()).toEqual([["set_eq_preset_bands", { preset: 0xa1, ult: 1, bands: SOUND.bands }]]);
    r.unmount();
  });

  it("a slot in the code names the target preset", async () => {
    const r = render(PasteCode, { st: connected(0xa0), onclose: () => {} });
    await type(r, encodeSound({ ...SOUND, slot: 2 }));
    await fireEvent.click(r.getByRole("button", { name: "Apply" }));
    await fireEvent.click(within(r.getByRole("dialog")).getByRole("button", { name: "Apply" }));
    await vi.waitFor(() => expect(writes()).toHaveLength(1));
    expect(writes()[0][1]).toMatchObject({ preset: 0xa2 });
    r.unmount();
  });

  it("a Sound code cannot be applied while disconnected or to an unlisted preset", async () => {
    const r = render(PasteCode, { st: emptyState(), onclose: () => {} });
    await type(r, encodeSound(SOUND));
    expect(r.getByRole("button", { name: "Apply" }).getAttribute("aria-disabled")).toBe("true");
    await fireEvent.click(r.getByRole("button", { name: "Apply" }));
    expect(r.queryByText("Apply this preset to the headphones?")).toBeNull();
    r.unmount();
    const u = render(PasteCode, { st: connected(0xa0, [0x00, 0xa0]), onclose: () => {} });
    await type(u, encodeSound({ ...SOUND, slot: 2 }));
    expect(u.getByText("the headphones do not list Custom 2")).toBeTruthy();
    expect(u.getByRole("button", { name: "Apply" }).getAttribute("aria-disabled")).toBe("true");
    u.unmount();
    expect(writes()).toEqual([]);
  });

  it("pasted_advanced_code_saves_only", async () => {
    const onsaved = vi.fn();
    const onclose = vi.fn();
    const bands = [-10, 10, -3, 4, 0, 7, -7, 2, 9, -1];
    // Works with no headphones at all: the curve lives in the app.
    const r = render(PasteCode, { st: emptyState(), onclose, onsaved });
    await type(r, encodeAdvanced(bands));
    expect(values(r.container)).toEqual(["−10", "+10", "−3", "+4", "0", "+7", "−7", "+2", "+9", "−1"]);
    expect(r.getByText("31 Hz")).toBeTruthy();
    await fireEvent.click(r.getByRole("button", { name: "Apply" }));
    await vi.waitFor(() => expect(onsaved).toHaveBeenCalledWith(bands));
    expect(calls("set_eq_curve")).toEqual([["set_eq_curve", { bands }]]);
    expect(writes()).toEqual([]);
    expect(onclose).toHaveBeenCalled();
    expect(r.queryByText("Apply this preset to the headphones?")).toBeNull();
    r.unmount();
  });

  it("Escape and Close leave without writing", async () => {
    const onclose = vi.fn();
    const r = render(PasteCode, { st: connected(), onclose });
    await type(r, encodeSound(SOUND));
    await fireEvent.keyDown(window, { key: "Escape" });
    expect(onclose).toHaveBeenCalledTimes(1);
    await fireEvent.click(r.getByRole("button", { name: "Close" }));
    expect(onclose).toHaveBeenCalledTimes(2);
    expect(writes()).toEqual([]);
    r.unmount();
  });
});
