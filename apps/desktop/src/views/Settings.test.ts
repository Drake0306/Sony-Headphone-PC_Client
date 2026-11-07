import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, waitFor } from "@testing-library/svelte";

const backend = vi.hoisted(() => ({
  settings: { theme: "paper", launch_at_login: false, hide_dock_icon: true, show_advanced: false },
  handler: null as ((e: { payload: unknown }) => void) | null,
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(async (cmd: string, args: { s?: typeof backend.settings }) => {
    if (cmd === "get_settings") return backend.settings;
    if (cmd === "set_settings") {
      backend.settings = args.s!;
      setTimeout(() => backend.handler?.({ payload: backend.settings }), 0);
    }
    if (cmd === "get_scenes") return [];
    return null;
  }),
}));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(async (name: string, h: (e: { payload: unknown }) => void) => {
    if (name === "settings") backend.handler = h;
    return () => {};
  }),
  emitTo: vi.fn(async () => {}),
}));
vi.mock("@tauri-apps/api/app", () => ({ getVersion: vi.fn(async () => "0.1.0") }));

import { invoke } from "@tauri-apps/api/core";
import { emptyState } from "../lib/state";
import Settings from "./Settings.svelte";

describe("Settings", () => {
  it("saves the Advanced panel switch with the other app settings", async () => {
    const r = render(Settings, { st: emptyState() });
    const toggle = r.getByLabelText("Advanced panel");
    await waitFor(() => expect(toggle).toHaveAttribute("aria-disabled", "false"));
    await fireEvent.click(toggle);
    await waitFor(() => expect(toggle).toHaveAttribute("aria-checked", "true"));
    const sent = vi.mocked(invoke).mock.calls.filter(([c]) => c === "set_settings").at(-1)?.[1];
    expect(sent).toEqual({ s: { theme: "paper", launch_at_login: false, hide_dock_icon: true, show_advanced: true } });
  });
});
