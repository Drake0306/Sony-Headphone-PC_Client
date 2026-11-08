import { beforeEach, describe, expect, it, vi } from "vitest";
import { get } from "svelte/store";

const io = vi.hoisted(() => ({
  handlers: {} as Record<string, (e: { payload: unknown }) => void>,
  status: { available: null as unknown, dismissed: null as string | null },
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(async (cmd: string) => {
    if (cmd === "current_version") return "0.2.0";
    if (cmd === "update_status") return io.status;
    return null;
  }),
}));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(async (name: string, h: (e: { payload: unknown }) => void) => {
    io.handlers[name] = h;
    return () => {};
  }),
  emitTo: vi.fn(async () => {}),
}));

import { invoke } from "@tauri-apps/api/core";
import { initial, later, percent, reduce, update, visible, type UpdatePhase } from "./update";

const available: UpdatePhase = { phase: "available", version: "0.2.1", notes: "Fixes", date: "2026-11-08" };
const send = (name: string, payload: unknown) => io.handlers[name]!({ payload });

describe("update reducer", () => {
  it("records an available release and keeps it through a failure", () => {
    let s = reduce(initial(), available);
    expect(s.available).toEqual({ version: "0.2.1", notes: "Fixes", date: "2026-11-08" });
    s = reduce(s, { phase: "failed", error: "Could not reach GitHub" });
    expect(s.available?.version).toBe("0.2.1");
    expect(s.phase).toEqual({ phase: "failed", error: "Could not reach GitHub" });
  });

  it("clears the release when up to date", () => {
    const s = reduce(reduce(initial(), available), { phase: "up_to_date" });
    expect(s.available).toBeNull();
    expect(s.phase?.phase).toBe("up_to_date");
  });

  it("does not let a background notice interrupt an install", () => {
    let s = reduce(reduce(initial(), available), { phase: "downloading", downloaded: 1, total: 4 });
    s = reduce(s, available);
    expect(s.phase?.phase).toBe("downloading");
  });

  it("works out the download percentage", () => {
    expect(percent({ phase: "downloading", downloaded: 42, total: 100 })).toBe(42);
    expect(percent({ phase: "downloading", downloaded: 7, total: null })).toBeNull();
    expect(percent({ phase: "downloading", downloaded: 150, total: 100 })).toBe(100);
    expect(percent({ phase: "installing" })).toBeNull();
  });
});

describe("update store", () => {
  beforeEach(() => {
    io.status = { available: null, dismissed: null };
  });

  it("starts from the running version and the app's last known release", async () => {
    io.status = { available: { version: "0.2.1", notes: "", date: null }, dismissed: null };
    const stop = update.subscribe(() => {});
    await vi.waitFor(() => expect(get(update).current).toBe("0.2.0"));
    await vi.waitFor(() => expect(get(visible)).toBe(true));
    stop();
  });

  it("follows the update events", async () => {
    const stop = update.subscribe(() => {});
    await vi.waitFor(() => expect(io.handlers["update"]).toBeDefined());
    send("update", available);
    expect(get(update).available?.version).toBe("0.2.1");
    send("update", { phase: "downloading", downloaded: 50, total: 100 });
    expect(percent(get(update).phase)).toBe(50);
    send("update", { phase: "installing" });
    expect(get(update).phase?.phase).toBe("installing");
    stop();
  });

  it("later_hides_that_version_until_relaunch", async () => {
    const stop = update.subscribe(() => {});
    await vi.waitFor(() => expect(io.handlers["update"]).toBeDefined());
    send("update", available);
    expect(get(visible)).toBe(true);
    await later();
    expect(get(visible)).toBe(false);
    expect(vi.mocked(invoke)).toHaveBeenCalledWith("dismiss_update", { version: "0.2.1" });
    // The six-hourly check finds the same version again: still hidden.
    send("update", available);
    expect(get(visible)).toBe(false);
    // Another window's Later reaches this one too.
    send("update-dismissed", null);
    expect(get(visible)).toBe(true);
    send("update-dismissed", "0.2.1");
    expect(get(visible)).toBe(false);
    // A newer version is shown.
    send("update", { ...available, version: "0.2.2" });
    expect(get(visible)).toBe(true);
    stop();
    // Nothing is written to storage: a relaunch starts with nothing dismissed.
    expect(localStorage.length).toBe(0);
  });
});
