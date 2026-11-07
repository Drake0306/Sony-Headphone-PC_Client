import { describe, expect, it, vi } from "vitest";
import { get } from "svelte/store";
import type { Scene } from "./scenes";

const io = vi.hoisted(() => ({
  reply: null as ((list: unknown[]) => void) | null,
  handler: null as ((e: { payload: unknown }) => void) | null,
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn((cmd: string) =>
    cmd === "get_scenes" ? new Promise((resolve) => (io.reply = resolve)) : Promise.resolve(),
  ),
}));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(async (_name: string, h: (e: { payload: unknown }) => void) => {
    io.handler = h;
    return () => {};
  }),
  emitTo: vi.fn(async () => {}),
}));

import { scenes } from "./scenes";

const scene = (id: string): Scene => ({
  id,
  name: id,
  enabled: true,
  trigger: { kind: "manual" },
  actions: { nc: null, ambient_level: null, voice_focus: null, ult: null, eq_preset: null, volume: null, dsee_auto_off: null },
  hotkey: null,
});

describe("scenes store", () => {
  it("scenes_store_drops_a_get_reply_older_than_an_event", async () => {
    const stop = scenes.subscribe(() => {});
    await vi.waitFor(() => expect(io.handler).not.toBeNull());
    io.handler!({ payload: { scenes: [scene("new")], active: "new" } });
    io.reply!([scene("old")]);
    await new Promise((done) => setTimeout(done, 0));
    expect(get(scenes)).toEqual({ scenes: [scene("new")], active: "new" });
    stop();
  });
});
