import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, waitFor } from "@testing-library/svelte";

const backend = vi.hoisted(() => ({
  settings: { theme: "paper", launch_at_login: false, hide_dock_icon: true, show_advanced: false, auto_check_updates: true },
  handler: null as ((e: { payload: unknown }) => void) | null,
  estimate: { minutes_left: null as number | null, samples: 0, since: null as number | null },
  battery: null as ((e: { payload: unknown }) => void) | null,
  update: null as ((e: { payload: unknown }) => void) | null,
  check: { phase: "up_to_date" } as Record<string, unknown> | string,
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(async (cmd: string, args: { s?: typeof backend.settings }) => {
    if (cmd === "get_settings") return backend.settings;
    if (cmd === "set_settings") {
      backend.settings = args.s!;
      setTimeout(() => backend.handler?.({ payload: backend.settings }), 0);
    }
    if (cmd === "get_scenes") return [];
    if (cmd === "current_version") return "0.2.0";
    if (cmd === "update_status") return { available: null, dismissed: null, phase: null, installing: false };
    if (cmd === "check_update") {
      if (typeof backend.check === "string") {
        setTimeout(() => backend.update?.({ payload: { phase: "failed", error: backend.check } }), 0);
        throw backend.check;
      }
      setTimeout(() => backend.update?.({ payload: backend.check }), 0);
    }
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
    if (name === "update") backend.update = h;
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
    expect(sent).toEqual({
      s: { theme: "paper", launch_at_login: false, hide_dock_icon: true, show_advanced: true, auto_check_updates: true },
    });
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

  it("auto_check_setting_round_trip", async () => {
    const r = render(Settings, { st: emptyState() });
    const toggle = r.getByLabelText("Check for updates automatically");
    await waitFor(() => expect(toggle).toHaveAttribute("aria-checked", "true"));
    await fireEvent.click(toggle);
    await waitFor(() => expect(toggle).toHaveAttribute("aria-checked", "false"));
    const sent = vi.mocked(invoke).mock.calls.filter(([c]) => c === "set_settings").at(-1)?.[1] as {
      s: typeof backend.settings;
    };
    expect(sent.s.auto_check_updates).toBe(false);
    r.unmount();
    // The next launch reads the saved file: still off.
    const next = render(Settings, { st: emptyState() });
    await waitFor(() =>
      expect(next.getByLabelText("Check for updates automatically")).toHaveAttribute("aria-checked", "false"),
    );
    next.unmount();
  });

  it("checks now and says up to date, or why not", async () => {
    const r = render(Settings, { st: emptyState() });
    await waitFor(() => expect(r.getByText("SHDC 0.2.0")).toBeInTheDocument());
    backend.check = { phase: "up_to_date" };
    await fireEvent.click(r.getByRole("button", { name: "Check now" }));
    await waitFor(() => expect(r.getByText("Up to date")).toBeInTheDocument());
    backend.check = "Could not reach GitHub";
    await fireEvent.click(r.getByRole("button", { name: "Check now" }));
    await waitFor(() => expect(r.getByText("Could not reach GitHub")).toBeInTheDocument());
    r.unmount();
  });

  it("offers a newer release and shows the install's progress", async () => {
    const r = render(Settings, { st: emptyState() });
    await waitFor(() => expect(backend.update).not.toBeNull());
    backend.update!({ payload: { phase: "available", version: "0.2.1", notes: "Fixes the tray", date: "2026-11-08" } });
    await waitFor(() => expect(r.getByText(/SHDC 0.2.1 is available/)).toBeInTheDocument());
    expect(r.getByText("Fixes the tray")).toBeInTheDocument();
    await fireEvent.click(r.getByRole("button", { name: "Update now" }));
    expect(vi.mocked(invoke)).toHaveBeenCalledWith("install_update");
    backend.update!({ payload: { phase: "downloading", downloaded: 42, total: 100 } });
    await waitFor(() => expect(r.getByText("Downloading… 42 %")).toBeInTheDocument());
    expect(r.getByRole("progressbar")).toHaveAttribute("aria-valuenow", "42");
    expect(r.queryByRole("button", { name: "Update now" })).toBeNull();
    backend.update!({ payload: { phase: "verifying" } });
    await waitFor(() => expect(r.getByText("Verifying…")).toBeInTheDocument());
    expect(r.queryByRole("button", { name: "Update now" })).toBeNull();
    backend.update!({ payload: { phase: "installing" } });
    await waitFor(() => expect(r.getByText("Installing…")).toBeInTheDocument());
    backend.update!({ payload: { phase: "restarting" } });
    await waitFor(() => expect(r.getByText("Restarting…")).toBeInTheDocument());
    r.unmount();
  });

  it("puts a release off with Later", async () => {
    const r = render(Settings, { st: emptyState() });
    await waitFor(() => expect(backend.update).not.toBeNull());
    backend.update!({ payload: { phase: "available", version: "0.2.3", notes: "", date: null } });
    await waitFor(() => expect(r.getByRole("button", { name: "Later" })).toBeInTheDocument());
    await fireEvent.click(r.getByRole("button", { name: "Later" }));
    await waitFor(() => expect(r.queryByText(/SHDC 0.2.3 is available/)).toBeNull());
    expect(vi.mocked(invoke)).toHaveBeenCalledWith("dismiss_update", { version: "0.2.3" });
    r.unmount();
  });
});
