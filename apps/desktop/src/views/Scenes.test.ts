import { beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, waitFor } from "@testing-library/svelte";
import type { Scene } from "../lib/scenes";

// A stand-in for the backend: set_scenes stores the list and emits "scenes".
const backend = vi.hoisted(() => ({
  list: [] as unknown[],
  active: null as string | null,
  reject: null as string | null,
  handler: null as ((e: { payload: unknown }) => void) | null,
}));
const emit = () => backend.handler?.({ payload: { scenes: backend.list, active: backend.active } });

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(async (cmd: string, args: { scenes?: unknown[]; id?: string }) => {
    if (cmd === "get_scenes") {
      setTimeout(emit, 0);
      return backend.list;
    }
    if (cmd === "set_scenes") {
      if (backend.reject) throw backend.reject;
      backend.list = args.scenes ?? [];
      setTimeout(emit, 0);
      return;
    }
    if (cmd === "apply_scene") {
      backend.active = args.id ?? null;
      setTimeout(emit, 0);
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
import Scenes from "./Scenes.svelte";

const scene = (id: string, name: string): Scene => ({
  id,
  name,
  enabled: true,
  trigger: { kind: "mic_in_use" },
  actions: { nc: "NoiseCancelling", ambient_level: null, voice_focus: null, ult: null, eq_preset: null, volume: null, dsee_auto_off: null },
  hotkey: null,
});
const ids = () => (backend.list as Scene[]).map((s) => s.id);
const lastSetScenes = () =>
  vi.mocked(invoke).mock.calls.filter(([c]) => c === "set_scenes").at(-1)?.[1] as { scenes: Scene[] } | undefined;

describe("Scenes", () => {
  beforeEach(() => {
    backend.list = [scene("a", "Alpha"), scene("b", "Beta")];
    backend.active = null;
    backend.reject = null;
    vi.mocked(invoke).mockClear();
  });

  it("scenes_new_scene_saves_the_whole_list", async () => {
    const r = render(Scenes);
    await waitFor(() => expect(r.getByText("Editing · Alpha")).toBeInTheDocument());
    await fireEvent.click(r.getByText("New scene", { selector: "button" }));
    await fireEvent.input(r.getByLabelText("Scene name"), { target: { value: "Late" } });
    await fireEvent.click(r.getByText("Time range"));
    await fireEvent.click(r.getByText("Save scene"));
    await waitFor(() => expect(ids()).toHaveLength(3));
    const sent = lastSetScenes()!.scenes;
    expect(sent.map((s) => s.name)).toEqual(["Alpha", "Beta", "Late"]);
    expect(sent[2].trigger).toEqual({ kind: "time_range", from: "22:00", to: "07:00" });
    expect(sent[2].enabled).toBe(false);
    await waitFor(() => expect(r.getByText("Editing · Late")).toBeInTheDocument());
  });

  it("scenes_delete_removes_and_opens_a_neighbour", async () => {
    const r = render(Scenes);
    await waitFor(() => expect(r.getByText("Editing · Alpha")).toBeInTheDocument());
    await fireEvent.click(r.getByText("Delete"));
    await waitFor(() => expect(ids()).toEqual(["b"]));
    expect(r.getByText("Editing · Beta")).toBeInTheDocument();
  });

  it("scenes_up_moves_the_scene_and_saves_the_order", async () => {
    const r = render(Scenes);
    await waitFor(() => expect(r.getByText("Editing · Alpha")).toBeInTheDocument());
    await fireEvent.click(r.getByLabelText("Move Beta up"));
    await waitFor(() => expect(ids()).toEqual(["b", "a"]));
  });

  it("scenes_backend_error_renders_in_the_note", async () => {
    backend.reject = 'scene "Alpha": could not save scenes: disk full';
    const r = render(Scenes);
    await waitFor(() => expect(r.getByText("Editing · Alpha")).toBeInTheDocument());
    await fireEvent.click(r.getByText("Save scene"));
    await waitFor(() => expect(r.getByText(backend.reject!)).toHaveClass("note", "live"));
  });

  it("scenes_local_validation_stops_before_the_round_trip", async () => {
    const r = render(Scenes);
    await waitFor(() => expect(r.getByText("Editing · Alpha")).toBeInTheDocument());
    await fireEvent.input(r.getByLabelText("Scene name"), { target: { value: " " } });
    await fireEvent.click(r.getByText("Save scene"));
    expect(r.getByText('scene " ": name is empty')).toHaveClass("live");
    expect(lastSetScenes()).toBeUndefined();
  });
});
