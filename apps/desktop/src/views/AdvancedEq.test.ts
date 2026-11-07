import { beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, waitFor } from "@testing-library/svelte";

const backend = vi.hoisted(() => ({
  curve: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0] as number[],
  /** When set, get_eq_curve never answers. */
  hang: false,
  previewFails: false,
  saveFails: false,
  calls: [] as { cmd: string; args?: Record<string, unknown> }[],
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(async (cmd: string, args?: Record<string, unknown>) => {
    backend.calls.push({ cmd, args });
    const { bestFitPreview } = await import("../lib/eqCurve");
    if (cmd === "get_eq_curve") {
      if (backend.hang) return new Promise(() => {});
      return { bands: [...backend.curve] };
    }
    if (cmd === "set_eq_curve") {
      if (backend.saveFails) throw "disk full";
      backend.curve = [...(args!.bands as number[])];
      return null;
    }
    if (cmd === "best_fit_preview") {
      if (backend.previewFails) throw "no preview";
      return bestFitPreview(backend.curve);
    }
    if (cmd === "never") return bestFitPreview(backend.curve);
    if (cmd === "send_best_fit") return bestFitPreview(backend.curve);
    return null;
  }),
}));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(async () => () => {}),
  emitTo: vi.fn(async () => {}),
}));

import { bestFitPreview, toRaw } from "../lib/eqCurve";
import { emptyState, type DeviceState, type Status } from "../lib/state";
import AdvancedEq from "./AdvancedEq.svelte";

const NOTE =
  "These bands run in Hairline only. The headphones keep their six-band curve; use Send best fit to put the closest match on them.";

const connected = (status: Status = "Confirmed", bands = [10, 10, 10, 10, 10, 10]): DeviceState => ({
  ...emptyState(),
  connected: true,
  eq_ult: { value: { preset: 0xa0, ult: "Off", bands }, status },
  eq_capability: { bands: 6, steps: 21, ult2_additional_step: 0, presets: [] },
});

const sliderLabels = (c: HTMLElement) =>
  [...c.querySelectorAll("[role=slider]")].map((s) => s.getAttribute("aria-label"));
const fitTexts = (c: HTMLElement) => [...c.querySelectorAll(".fit .best .dot")].map((n) => n.textContent);
const nowTexts = (c: HTMLElement) => [...c.querySelectorAll(".fit .now .dot")].map((n) => n.textContent);
const levelText = (v: number) => (v > 0 ? `+${v}` : v < 0 ? `−${-v}` : "0");
const calls = (cmd: string) => backend.calls.filter((c) => c.cmd === cmd);

describe("Advanced equalizer", () => {
  beforeEach(() => {
    backend.curve = [6, 3, 0, -2, 0, 0, 0, 0, 0, 0];
    backend.hang = false;
    backend.previewFails = false;
    backend.saveFails = false;
    backend.calls = [];
  });

  it("paging_bounds", async () => {
    const r = render(AdvancedEq, { st: connected(), onback: () => {} });
    await waitFor(() => expect(r.container.querySelectorAll("[role=slider]")).toHaveLength(4));
    const prev = r.getByRole("button", { name: "Previous bands" });
    const next = r.getByRole("button", { name: "Next bands" });
    const strip = () =>
      [...r.container.querySelectorAll(".strip .bands span")].map((s) => s.classList.contains("on"));
    const onAt = (from: number) => Array.from({ length: 10 }, (_, i) => i >= from && i < from + 4);

    expect(r.getByText("Bands 1–4 of 10")).toBeTruthy();
    expect(sliderLabels(r.container)).toEqual(["31 Hz", "62 Hz", "125 Hz", "250 Hz"]);
    expect(strip()).toEqual(onAt(0));
    expect(prev.getAttribute("aria-disabled")).toBe("true");
    await fireEvent.click(prev);
    expect(r.getByText("Bands 1–4 of 10")).toBeTruthy();

    await fireEvent.click(next);
    expect(r.getByText("Bands 5–8 of 10")).toBeTruthy();
    expect(sliderLabels(r.container)).toEqual(["500 Hz", "1 kHz", "2 kHz", "4 kHz"]);
    expect(strip()).toEqual(onAt(4));

    // The focused fader keeps its focus when it stays on the next page.
    const twoK = r.getByRole("slider", { name: "2 kHz" });
    twoK.focus();
    await fireEvent.click(next);
    expect(r.getByText("Bands 7–10 of 10")).toBeTruthy();
    expect(sliderLabels(r.container)).toEqual(["2 kHz", "4 kHz", "8 kHz", "16 kHz"]);
    expect(strip()).toEqual(onAt(6));
    expect(document.activeElement).toBe(twoK);
    expect(next.getAttribute("aria-disabled")).toBe("true");

    // Never past the last page, never an empty page.
    await fireEvent.click(next);
    expect(r.getByText("Bands 7–10 of 10")).toBeTruthy();
    expect(r.container.querySelectorAll("[role=slider]")).toHaveLength(4);

    await fireEvent.click(prev);
    expect(r.getByText("Bands 5–8 of 10")).toBeTruthy();
    r.unmount();
  });

  it("curve_persists_on_change", async () => {
    const r = render(AdvancedEq, { st: connected(), onback: () => {} });
    const low = await r.findByRole("slider", { name: "31 Hz" });
    expect(low.getAttribute("aria-valuenow")).toBe(String(toRaw(6)));
    expect(fitTexts(r.container)).toEqual(bestFitPreview([6, 3, 0, -2, 0, 0, 0, 0, 0, 0]).map(levelText));

    // 250 Hz up by 4 (−2 → +2): the fitted numbers follow before anything is saved.
    const mid = r.getByRole("slider", { name: "250 Hz" });
    await fireEvent.keyDown(mid, { key: "ArrowUp" });
    await fireEvent.keyDown(mid, { key: "ArrowUp" });
    await fireEvent.keyDown(mid, { key: "ArrowUp" });
    await fireEvent.keyDown(mid, { key: "ArrowUp" });
    const edited = [6, 3, 0, 2, 0, 0, 0, 0, 0, 0];
    expect(fitTexts(r.container)).toEqual(bestFitPreview(edited).map(levelText));
    expect(calls("set_eq_curve")).toHaveLength(0);

    // The commit saves the whole ten-band curve, then asks for the stored fit.
    await waitFor(() => expect(calls("set_eq_curve")).toHaveLength(1));
    expect(calls("set_eq_curve")[0].args).toEqual({ bands: edited });
    await waitFor(() => expect(calls("best_fit_preview")).toHaveLength(1));
    const order = backend.calls.map((c) => c.cmd);
    expect(order.indexOf("best_fit_preview")).toBeGreaterThan(order.indexOf("set_eq_curve"));
    // Nothing went to the headphones.
    expect(calls("send_best_fit")).toHaveLength(0);
    expect(calls("set_eq_bands")).toHaveLength(0);

    // Leaving and coming back reads the saved curve.
    r.unmount();
    const again = render(AdvancedEq, { st: connected(), onback: () => {} });
    const back = await again.findByRole("slider", { name: "250 Hz" });
    expect(back.getAttribute("aria-valuenow")).toBe(String(toRaw(2)));
    again.unmount();
  });

  it("send_best_fit_invokes_command", async () => {
    const r = render(AdvancedEq, { st: connected(), onback: () => {} });
    await r.findByRole("slider", { name: "31 Hz" });
    const send = r.getByRole("button", { name: "Send best fit to headphones" });
    expect(send.getAttribute("aria-disabled")).toBe("false");
    await fireEvent.click(send);
    await waitFor(() => expect(calls("send_best_fit")).toHaveLength(1));
    expect(calls("set_eq_bands")).toHaveLength(0);

    // The device answers with the fitted bands: "Sent".
    const fit = bestFitPreview(backend.curve).map(toRaw);
    await r.rerender({ st: connected({ Pending: { since_ms: 1 } }), onback: () => {} });
    expect(r.getByRole("button", { name: /Send best fit/ }).getAttribute("aria-disabled")).toBe("true");
    await r.rerender({ st: connected("Confirmed", fit), onback: () => {} });
    await waitFor(() => expect(r.getByText("Sent")).toBeTruthy());
    r.unmount();
  });

  it("send shows the refusal and is unavailable while disconnected", async () => {
    const r = render(AdvancedEq, { st: connected(), onback: () => {} });
    await r.findByRole("slider", { name: "31 Hz" });
    await fireEvent.click(r.getByRole("button", { name: "Send best fit to headphones" }));
    await waitFor(() => expect(calls("send_best_fit")).toHaveLength(1));
    await r.rerender({ st: connected({ Pending: { since_ms: 1 } }), onback: () => {} });
    await r.rerender({ st: connected({ Refused: { reason: "Not now" } }), onback: () => {} });
    await waitFor(() => expect(r.getByText("Not now")).toBeTruthy());

    await r.rerender({ st: emptyState(), onback: () => {} });
    const send = r.getByRole("button", { name: "Send best fit to headphones" });
    expect(send.getAttribute("aria-disabled")).toBe("true");
    const before = calls("send_best_fit").length;
    await fireEvent.click(send);
    await new Promise((done) => setTimeout(done, 10));
    expect(calls("send_best_fit")).toHaveLength(before);
    // The curve stays editable without headphones.
    expect(r.getByRole("slider", { name: "31 Hz" }).getAttribute("aria-disabled")).toBe("false");
    r.unmount();
  });

  it("shows the headphones' current bands beside the best fit", async () => {
    const r = render(AdvancedEq, { st: connected("Confirmed", [16, 12, 10, 7, 3, 20]), onback: () => {} });
    await r.findByRole("slider", { name: "31 Hz" });
    expect(r.getByText("On the headphones now")).toBeTruthy();
    expect(r.getByText("Best fit")).toBeTruthy();
    expect(nowTexts(r.container)).toEqual(["+6", "+2", "0", "−3", "−7", "+10"]);
    expect(fitTexts(r.container)).toHaveLength(6);
    await r.rerender({ st: connected("Confirmed", [10, 10, 11, 10, 10, 0]), onback: () => {} });
    expect(nowTexts(r.container)).toEqual(["0", "0", "+1", "0", "0", "−10"]);
    await r.rerender({ st: { ...connected("Confirmed", [16, 12, 10, 7, 3, 20]), connected: false }, onback: () => {} });
    expect(nowTexts(r.container)).toEqual(Array(6).fill("--"));
    await r.rerender({ st: emptyState(), onback: () => {} });
    expect(nowTexts(r.container)).toEqual(Array(6).fill("--"));
    r.unmount();
  });

  it("a failed preview after a save does not say the curve was not saved", async () => {
    backend.previewFails = true;
    const r = render(AdvancedEq, { st: connected(), onback: () => {} });
    const low = await r.findByRole("slider", { name: "31 Hz" });
    await fireEvent.keyDown(low, { key: "ArrowUp" });
    await waitFor(() => expect(r.getByText("Saved; preview unavailable")).toBeTruthy());
    expect(r.queryByText(/Not saved/)).toBeNull();
    backend.previewFails = false;
    backend.saveFails = true;
    await fireEvent.keyDown(low, { key: "ArrowUp" });
    await waitFor(() => expect(r.getByText(/^Not saved/)).toBeTruthy());
    backend.saveFails = false;
    r.unmount();
  });

  it("back returns to Sound", async () => {
    const onback = vi.fn();
    const r = render(AdvancedEq, { st: connected(), onback });
    await fireEvent.click(r.getByRole("button", { name: "‹ Sound" }));
    expect(onback).toHaveBeenCalledTimes(1);
    r.unmount();
  });

  it("shows the note verbatim and the device labels", async () => {
    const r = render(AdvancedEq, { st: connected(), onback: () => {} });
    expect(r.getByText(NOTE).classList.contains("live")).toBe(true);
    expect(r.getByText("Advanced equalizer")).toBeTruthy();
    expect(r.getByText("What the headphones will hold")).toBeTruthy();
    const labels = [...r.container.querySelectorAll(".fit .dev")].map((n) => n.textContent);
    expect(labels).toEqual(["Clear Bass", "400 Hz", "1 kHz", "2.5 kHz", "6.3 kHz", "16 kHz"]);
    r.unmount();
  });

  it("renders nothing from literals but the labels", async () => {
    backend.hang = true;
    const r = render(AdvancedEq, { st: connected(), onback: () => {} });
    await new Promise((done) => setTimeout(done, 10));
    // Before the curve arrives: no knob, no values, no fit, and no send.
    expect(r.container.querySelectorAll("[role=slider]")).toHaveLength(0);
    expect([...r.container.querySelectorAll(".fader .val")].map((n) => n.textContent)).toEqual(Array(4).fill("--"));
    expect(fitTexts(r.container)).toEqual(Array(6).fill("--"));
    expect(r.getByRole("button", { name: "Send best fit to headphones" }).getAttribute("aria-disabled")).toBe("true");
    r.unmount();

    backend.hang = false;
    backend.curve = [-4, -4, -4, -4, 5, -9, 1, 7, -3, 8];
    const b = render(AdvancedEq, { st: connected(), onback: () => {} });
    await b.findByRole("slider", { name: "31 Hz" });
    expect([...b.container.querySelectorAll(".fader .val")].map((n) => n.textContent)).toEqual(["−4", "−4", "−4", "−4"]);
    expect(fitTexts(b.container)).toEqual(bestFitPreview(backend.curve).map(levelText));
    b.unmount();
  });
});
