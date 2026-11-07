import { beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, waitFor } from "@testing-library/svelte";
import type { Scene } from "../lib/scenes";

const backend = vi.hoisted(() => ({
  list: [] as unknown[],
  active: null as string | null,
  reject: null as string | null,
  emitOnApply: true,
  handler: null as ((e: { payload: unknown }) => void) | null,
}));
const emit = () => backend.handler?.({ payload: { scenes: backend.list, active: backend.active } });

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(async (cmd: string, args: { id?: string }) => {
    if (cmd === "get_scenes") {
      setTimeout(emit, 0);
      return backend.list;
    }
    if (cmd === "get_state") {
      const { emptyState } = await import("../lib/state");
      return { ...emptyState(), connected: true, model: "ULT WEAR" };
    }
    if (cmd === "runtime_info") return { fake: true, version: "0" };
    if (cmd === "apply_scene") {
      if (backend.reject) throw backend.reject;
      backend.active = args.id ?? null;
      if (backend.emitOnApply) setTimeout(emit, 0);
    }
  }),
}));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(async (name: string, h: (e: { payload: unknown }) => void) => {
    if (name === "scenes") backend.handler = h;
    return () => {};
  }),
  emitTo: vi.fn(async () => {}),
}));

import { invoke } from "@tauri-apps/api/core";
import Popover from "./Popover.svelte";

const scene = (id: string, enabled = true): Scene => ({
  id,
  name: id.toUpperCase(),
  enabled,
  trigger: { kind: "manual" },
  actions: { nc: null, ambient_level: null, voice_focus: null, ult: null, eq_preset: null, volume: null, dsee_auto_off: null },
  hotkey: null,
});
const tick = () => new Promise((done) => setTimeout(done, 10));
const modeBoxes = (mode: HTMLElement) =>
  [...mode.parentElement!.querySelectorAll("button")].map((b) => b.textContent);

describe("Popover mode row", () => {
  beforeEach(() => {
    backend.active = null;
    backend.reject = null;
    backend.emitOnApply = true;
    vi.mocked(invoke).mockClear();
  });

  it("mode_row_hidden_without_an_enabled_scene", async () => {
    backend.list = [scene("a", false), scene("b", false)];
    const r = render(Popover);
    await waitFor(() => expect(vi.mocked(invoke).mock.calls.some(([c]) => c === "get_scenes")).toBe(true));
    await waitFor(() => expect(r.getByText("ULT WEAR")).toBeInTheDocument());
    expect(r.queryByText("Mode")).toBeNull();
  });

  it("mode_row_shows_four_and_more_with_five_enabled", async () => {
    backend.list = ["a", "b", "c", "d", "e"].map((id) => scene(id));
    const r = render(Popover);
    await waitFor(() => expect(r.getByText("Mode")).toBeInTheDocument());
    await waitFor(() => expect(r.getByText("Battery")).toBeInTheDocument());
    await tick(); // the "scenes" event that follows get_scenes
    expect(modeBoxes(r.getByText("Mode"))).toEqual(["A", "B", "C", "D", "…"]);
    await fireEvent.click(r.getByText("…"));
    await waitFor(() => expect(vi.mocked(invoke).mock.calls.some(([c]) => c === "open_main_window")).toBe(true));
  });

  it("mode_click_applies_and_stays_pending_until_the_event", async () => {
    backend.list = [scene("a"), scene("b")];
    backend.emitOnApply = false;
    const r = render(Popover);
    await waitFor(() => expect(r.getByText("Mode")).toBeInTheDocument());
    await waitFor(() => expect(r.getByText("Battery")).toBeInTheDocument());
    await tick(); // the "scenes" event that follows get_scenes
    await fireEvent.click(r.getByText("B"));
    expect(vi.mocked(invoke)).toHaveBeenCalledWith("apply_scene", { id: "b" });
    const row = r.getByText("B").closest(".boxes")!;
    await tick();
    expect(row).toHaveClass("pending");
    emit();
    await waitFor(() => expect(row).not.toHaveClass("pending"));
    expect(r.getByText("B")).toHaveClass("on");
  });

  it("mode_failed_apply_is_refused_on_that_box", async () => {
    backend.list = [scene("a"), scene("b")];
    backend.reject = 'no scene "b"';
    const r = render(Popover);
    await waitFor(() => expect(r.getByText("Mode")).toBeInTheDocument());
    await waitFor(() => expect(r.getByText("Battery")).toBeInTheDocument());
    await tick(); // the "scenes" event that follows get_scenes
    await fireEvent.click(r.getByText("B"));
    await waitFor(() => expect(r.getByText("B")).toHaveClass("refused"));
    expect(r.getByText("B")).toHaveAttribute("title", 'no scene "b"');
    expect(r.getByText("A")).not.toHaveClass("refused");
  });
});
