import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, waitFor } from "@testing-library/svelte";

const backend = vi.hoisted(() => ({
  settings: { theme: "paper", launch_at_login: false, hide_dock_icon: true, show_advanced: false },
  handler: null as ((e: { payload: unknown }) => void) | null,
  estimate: { minutes_left: null as number | null, samples: 0, since: null as number | null },
  battery: null as ((e: { payload: unknown }) => void) | null,
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(async (cmd: string, args: { s?: typeof backend.settings }) => {
    if (cmd === "get_settings") return backend.settings;
    if (cmd === "set_settings") {
      backend.settings = args.s!;
      setTimeout(() => backend.handler?.({ payload: backend.settings }), 0);
    }
    if (cmd === "get_scenes") return [];
    if (cmd === "battery_estimate") return backend.estimate;
    if (cmd === "clear_battery_history")
      setTimeout(() => backend.battery?.({ payload: { minutes_left: null, samples: 0, since: null } }), 0);
    return null;
  }),
}));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(async (name: string, h: (e: { payload: unknown }) => void) => {
    if (name === "settings") backend.handler = h;
    if (name === "battery") backend.battery = h;
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

  it("shows the battery history and clears it", async () => {
    backend.estimate = { minutes_left: 300, samples: 12, since: new Date(2026, 10, 7).getTime() / 1000 };
    const r = render(Settings, { st: emptyState() });
    const date = new Date(2026, 10, 7).toLocaleDateString();
    await waitFor(() => expect(r.getByText(`12 samples since ${date}`)).toBeInTheDocument());
    await fireEvent.click(r.getByRole("button", { name: "Clear" }));
    expect(vi.mocked(invoke)).toHaveBeenCalledWith("clear_battery_history");
    await waitFor(() => expect(r.getByText("No samples yet")).toBeInTheDocument());
    expect(r.queryByRole("button", { name: "Clear" })).toBeNull();
    r.unmount();
  });
});
