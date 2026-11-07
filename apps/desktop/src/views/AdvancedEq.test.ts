import { beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, waitFor, within } from "@testing-library/svelte";

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
    if (cmd === "send_best_fit") {
      backend.curve = [...(args!.bands as number[])];
      return bestFitPreview(backend.curve);
    }
    return null;
  }),
}));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(async () => () => {}),
  emitTo: vi.fn(async () => {}),
}));

import { FADER_SIZES } from "../lib/controls/Fader.svelte";
import faderSrc from "../lib/controls/Fader.svelte?raw";
import { bestFitPreview, toRaw } from "../lib/eqCurve";
import { emptyState, type DeviceState, type Status } from "../lib/state";
import AdvancedEq from "./AdvancedEq.svelte";
import advSrc from "./AdvancedEq.svelte?raw";

const NOTE =
  "These bands run in Hairline only. The headphones keep their six-band curve; use Send best fit to put the closest match on them.";

const MANUAL = 0xa0;
const CUSTOM_1 = 0xa1;
const BRIGHT = 0x10;

const connected = (status: Status = "Confirmed", bands = [10, 10, 10, 10, 10, 10], preset = MANUAL): DeviceState => ({
  ...emptyState(),
  connected: true,
  eq_ult: { value: { preset, ult: "Off", bands }, status },
  eq_capability: { bands: 6, steps: 21, ult2_additional_step: 0, presets: [] },
});

const sliderLabels = (c: HTMLElement) =>
  [...c.querySelectorAll("[role=slider]")].map((s) => s.getAttribute("aria-label"));
const fitTexts = (c: HTMLElement) => [...c.querySelectorAll(".fit .best .dot")].map((n) => n.textContent);
const nowTexts = (c: HTMLElement) => [...c.querySelectorAll(".fit .now .dot")].map((n) => n.textContent);
const levelText = (v: number) => (v > 0 ? `+${v}` : v < 0 ? `−${-v}` : "0");
const calls = (cmd: string) => backend.calls.filter((c) => c.cmd === cmd);
const deviceWrites = () => backend.calls.filter((c) => c.cmd === "send_best_fit" || c.cmd === "set_eq_bands");
const body = (target: string) =>
  `The headphones hold six bands (Clear Bass, 400 Hz, 1 kHz, 2.5 kHz, 6.3 kHz, 16 kHz), not ten. Hairline fits your ten bands onto those six and writes them into ${target}.`;

/** The declarations of the first `selector { ... }` rule in a component's styles. */
const rule = (css: string, selector: string) => {
  const at = css.indexOf(`\n  ${selector} {`);
  expect(at, selector).toBeGreaterThanOrEqual(0);
  return css.slice(at, css.indexOf("}", at));
};

/** Opens the confirmation from the Send button and confirms it. */
async function confirmSend(container: HTMLElement) {
  const r = within(container);
  await fireEvent.click(r.getByRole("button", { name: "Send best fit to headphones" }));
  const dialog = await r.findByRole("dialog");
  await fireEvent.click(within(dialog).getByRole("button", { name: "Send" }));
}

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
    await confirmSend(r.container);
    await waitFor(() => expect(calls("send_best_fit")).toHaveLength(1));
    expect(calls("send_best_fit")[0].args).toEqual({ bands: [6, 3, 0, -2, 0, 0, 0, 0, 0, 0], preset: MANUAL });
    expect(r.queryByRole("dialog")).toBeNull();
    expect(calls("set_eq_bands")).toHaveLength(0);

    // The device answers with the fitted bands: "Sent".
    const fit = bestFitPreview(backend.curve).map(toRaw);
    await r.rerender({ st: connected({ Pending: { since_ms: 1 } }), onback: () => {} });
    expect(r.getByRole("button", { name: /Send best fit/ }).getAttribute("aria-disabled")).toBe("true");
    // While the send is pending the faders are inert and faded.
    const low = r.getByRole("slider", { name: "31 Hz" });
    expect(low.getAttribute("aria-disabled")).toBe("true");
    expect(r.container.querySelector(".faders")!.classList.contains("pending")).toBe(true);
    await fireEvent.keyDown(low, { key: "ArrowUp" });
    expect(low.getAttribute("aria-valuenow")).toBe(String(toRaw(6)));
    await r.rerender({ st: connected("Confirmed", fit), onback: () => {} });
    await waitFor(() => expect(r.getByText("Sent")).toBeTruthy());
    r.unmount();
  });

  it("send shows the refusal and is unavailable while disconnected", async () => {
    const r = render(AdvancedEq, { st: connected(), onback: () => {} });
    await r.findByRole("slider", { name: "31 Hz" });
    await confirmSend(r.container);
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
    expect(r.queryByRole("dialog")).toBeNull();
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
    expect(r.getByText("Saved; preview unavailable").classList.contains("live")).toBe(false);
    expect(r.queryByText(/Not saved/)).toBeNull();
    backend.previewFails = false;
    backend.saveFails = true;
    await fireEvent.keyDown(low, { key: "ArrowUp" });
    await waitFor(() => expect(r.getByText(/^Not saved/)).toBeTruthy());
    backend.saveFails = false;
    r.unmount();
  });

  it("send needs six bands of 21 steps", async () => {
    const st = connected();
    const r = render(AdvancedEq, {
      st: { ...st, eq_capability: { bands: 5, steps: 21, ult2_additional_step: 0, presets: [] } },
      onback: () => {},
    });
    await r.findByRole("slider", { name: "31 Hz" });
    const note = r.getByText("this headphone's equalizer has 5 bands / 21 steps; best fit needs 6 and 21");
    expect(note.classList.contains("live")).toBe(false);
    const send = r.getByRole("button", { name: "Send best fit to headphones" });
    expect(send.getAttribute("aria-disabled")).toBe("true");
    await fireEvent.click(send);
    await new Promise((done) => setTimeout(done, 10));
    expect(calls("send_best_fit")).toHaveLength(0);
    // Unknown capability is allowed.
    await r.rerender({ st: { ...st, eq_capability: null }, onback: () => {} });
    expect(send.getAttribute("aria-disabled")).toBe("false");
    expect(r.queryByText(/best fit needs/)).toBeNull();
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

  it("send_asks_first_and_names_the_target", async () => {
    const r = render(AdvancedEq, { st: connected(), onback: () => {} });
    await r.findByRole("slider", { name: "31 Hz" });
    expect(r.queryByRole("dialog")).toBeNull();
    await fireEvent.click(r.getByRole("button", { name: "Send best fit to headphones" }));
    const dialog = await r.findByRole("dialog");
    expect(dialog.getAttribute("aria-modal")).toBe("true");
    expect(within(dialog).getByText("Send best fit to the headphones?")).toBeTruthy();
    expect(within(dialog).getByText(body("Manual"))).toBeTruthy();
    // The six fitted values with their labels.
    const rows = [...dialog.querySelectorAll(".details > div")].map((d) => [
      d.querySelector("dt")!.textContent,
      d.querySelector("dd")!.textContent,
    ]);
    const labels = ["Clear Bass", "400 Hz", "1 kHz", "2.5 kHz", "6.3 kHz", "16 kHz"];
    const fit = bestFitPreview([6, 3, 0, -2, 0, 0, 0, 0, 0, 0]).map(levelText);
    expect(rows).toEqual(labels.map((l, k) => [l, fit[k]]));
    // Focus starts on Cancel; nothing has been sent yet.
    expect(document.activeElement).toBe(within(dialog).getByRole("button", { name: "Cancel" }));
    expect(deviceWrites()).toHaveLength(0);
    r.unmount();

    // From a fixed preset the fit goes into Custom 1 and the dialog says so.
    const b = render(AdvancedEq, { st: connected("Confirmed", undefined, BRIGHT), onback: () => {} });
    await b.findByRole("slider", { name: "31 Hz" });
    await fireEvent.click(b.getByRole("button", { name: "Send best fit to headphones" }));
    const d2 = await b.findByRole("dialog");
    expect(within(d2).getByText(body("Custom 1 (the headphones will switch to it)"))).toBeTruthy();
    await fireEvent.click(within(d2).getByRole("button", { name: "Send" }));
    await waitFor(() => expect(calls("send_best_fit")).toHaveLength(1));
    expect(calls("send_best_fit")[0].args).toEqual({ bands: [6, 3, 0, -2, 0, 0, 0, 0, 0, 0], preset: CUSTOM_1 });
    expect(calls("set_eq_bands")).toHaveLength(0);
    b.unmount();
  });

  it("cancel_sends_nothing", async () => {
    const r = render(AdvancedEq, { st: connected("Confirmed", undefined, BRIGHT), onback: () => {} });
    await r.findByRole("slider", { name: "31 Hz" });
    const open = async () => {
      await fireEvent.click(r.getByRole("button", { name: "Send best fit to headphones" }));
      return r.findByRole("dialog");
    };
    // The Cancel button.
    let dialog = await open();
    await fireEvent.click(within(dialog).getByRole("button", { name: "Cancel" }));
    expect(r.queryByRole("dialog")).toBeNull();
    // Escape.
    dialog = await open();
    await fireEvent.keyDown(dialog, { key: "Escape" });
    expect(r.queryByRole("dialog")).toBeNull();
    // A click on the backdrop; a click inside the panel keeps it open.
    dialog = await open();
    await fireEvent.click(within(dialog).getByText("Send best fit to the headphones?"));
    expect(r.queryByRole("dialog")).not.toBeNull();
    await fireEvent.click(r.container.querySelector(".backdrop")!);
    expect(r.queryByRole("dialog")).toBeNull();
    await new Promise((done) => setTimeout(done, 10));
    expect(deviceWrites()).toHaveLength(0);
    r.unmount();
  });

  it("reset_zeroes_and_saves", async () => {
    const r = render(AdvancedEq, { st: connected(), onback: () => {} });
    await r.findByRole("slider", { name: "31 Hz" });
    const reset = r.getByRole("button", { name: "Reset" });
    expect(reset.getAttribute("aria-disabled")).toBe("false");
    await fireEvent.click(reset);
    // All ten bands at 0 on the faders and in the fit, saved through set_eq_curve.
    expect(sliderLabels(r.container)).toEqual(["31 Hz", "62 Hz", "125 Hz", "250 Hz"]);
    for (const s of r.container.querySelectorAll("[role=slider]")) expect(s.getAttribute("aria-valuenow")).toBe(String(toRaw(0)));
    expect(fitTexts(r.container)).toEqual(Array(6).fill("0"));
    await waitFor(() => expect(calls("set_eq_curve")).toHaveLength(1));
    expect(calls("set_eq_curve")[0].args).toEqual({ bands: Array(10).fill(0) });
    await waitFor(() => expect(calls("best_fit_preview")).toHaveLength(1));
    expect(backend.curve).toEqual(Array(10).fill(0));
    expect(deviceWrites()).toHaveLength(0);

    // Unavailable while a send is pending.
    backend.curve = [6, 3, 0, -2, 0, 0, 0, 0, 0, 0];
    await r.rerender({ st: connected({ Pending: { since_ms: 1 } }), onback: () => {} });
    expect(reset.getAttribute("aria-disabled")).toBe("true");
    await fireEvent.click(reset);
    await new Promise((done) => setTimeout(done, 10));
    expect(calls("set_eq_curve")).toHaveLength(1);
    r.unmount();
  });

  it("advanced_layout_rules", () => {
    // 880×560: tighter page padding, and the faders take the free height.
    expect(rule(advSrc, ".adv")).toMatch(/padding:\s*20px 32px 16px/);
    const faders = rule(advSrc, ".faders");
    expect(faders).toMatch(/flex:\s*1;/);
    expect(faders).toMatch(/min-height:\s*300px/);
    const gap = Number(/\bgap:\s*(\d+)px/.exec(faders)?.[1]);
    expect(gap).toBeGreaterThanOrEqual(24);
    // No fixed track height at any size: the rail fills its column.
    expect(rule(faderSrc, ".rail")).not.toMatch(/(^|[^-])height\s*:/);
    expect(rule(faderSrc, ".rail")).toMatch(/flex:\s*1;/);
    expect(rule(faderSrc, ".fader")).toMatch(/height:\s*100%/);
    for (const size of Object.values(FADER_SIZES)) expect(Object.keys(size).some((k) => /height/i.test(k))).toBe(false);
    // One compact bottom row: the mini table at 11px mono beside the actions.
    const bottom = rule(advSrc, ".bottom");
    expect(bottom).toMatch(/display:\s*flex/);
    expect(bottom).not.toMatch(/flex-wrap:\s*wrap/);
    const fit = rule(advSrc, ".fit");
    expect(fit).toMatch(/font-family:\s*"DM Mono"/);
    expect(fit).toMatch(/font-size:\s*11px/);
    expect(fit).toMatch(/grid-template-columns:\s*auto repeat\(6,/);
    // Nothing scrolls.
    expect(advSrc).not.toMatch(/overflow(-y)?\s*:\s*(auto|scroll)/);
  });
});
