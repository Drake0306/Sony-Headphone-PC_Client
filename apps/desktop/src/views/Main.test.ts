import { beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, waitFor } from "@testing-library/svelte";

const backend = vi.hoisted(() => ({ showAdvanced: false }));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(async (cmd: string) => {
    if (cmd === "get_settings")
      return { theme: "paper", launch_at_login: false, hide_dock_icon: true, show_advanced: backend.showAdvanced };
    if (cmd === "get_state") {
      const { emptyState } = await import("../lib/state");
      return emptyState();
    }
    if (cmd === "runtime_info") return { fake: true, version: "0" };
    if (cmd === "capability_list") return [];
    if (cmd === "get_scenes") return [];
    return null;
  }),
}));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(async () => () => {}),
  emitTo: vi.fn(async () => {}),
}));

import { listen } from "@tauri-apps/api/event";
import Main from "./Main.svelte";

const navLabels = (c: HTMLElement) => [...c.querySelectorAll("nav a")].map((a) => a.textContent);

describe("Main nav", () => {
  beforeEach(() => {
    location.hash = "";
  });

  it("has no Advanced tab while the setting is off", async () => {
    backend.showAdvanced = false;
    const r = render(Main);
    await waitFor(() => expect(navLabels(r.container)).toContain("Settings"));
    await new Promise((done) => setTimeout(done, 10));
    expect(navLabels(r.container)).not.toContain("Advanced");
    r.unmount();
  });

  it("shows Advanced last when the setting is on", async () => {
    backend.showAdvanced = true;
    const r = render(Main);
    await waitFor(() => expect(navLabels(r.container).at(-1)).toBe("Advanced"));
    r.unmount();
  });
});

describe("Advanced equalizer from Sound", () => {
  beforeEach(() => {
    location.hash = "#sound";
  });

  it("back returns to Sound", async () => {
    backend.showAdvanced = false;
    const r = render(Main);
    const open = await r.findByRole("button", { name: "Advanced" });
    await fireEvent.click(open);
    expect(r.getByText("Advanced equalizer")).toBeTruthy();
    // The view takes the whole content area: no nav, and no hash of its own.
    expect(r.container.querySelector("nav")).toBeNull();
    expect(r.container.querySelector(".eq")).toBeNull();
    expect(location.hash).toBe("#sound");

    await fireEvent.click(r.getByRole("button", { name: "‹ Sound" }));
    expect(r.queryByText("Advanced equalizer")).toBeNull();
    expect(r.container.querySelector(".eq")).not.toBeNull();
    expect(navLabels(r.container)).toContain("Sound");
    r.unmount();
  });

  it("a tab change closes it", async () => {
    const r = render(Main);
    await fireEvent.click(await r.findByRole("button", { name: "Advanced" }));
    expect(r.getByText("Advanced equalizer")).toBeTruthy();
    location.hash = "#noise";
    window.dispatchEvent(new HashChangeEvent("hashchange"));
    await waitFor(() => expect(r.queryByText("Advanced equalizer")).toBeNull());
    location.hash = "#sound";
    window.dispatchEvent(new HashChangeEvent("hashchange"));
    await waitFor(() => expect(r.container.querySelector(".eq")).not.toBeNull());
    expect(r.queryByText("Advanced equalizer")).toBeNull();
    r.unmount();
  });
});

describe("Advanced equalizer and the popover", () => {
  beforeEach(() => {
    location.hash = "#sound";
  });

  it("a popover navigate closes it, even to Sound", async () => {
    const r = render(Main);
    await fireEvent.click(await r.findByRole("button", { name: "Advanced" }));
    expect(r.getByText("Advanced equalizer")).toBeTruthy();
    const calls = vi.mocked(listen).mock.calls.filter(([name]) => name === "navigate");
    const handler = calls.at(-1)![1] as (e: { payload: string }) => void;
    handler({ payload: "sound" });
    await waitFor(() => expect(r.queryByText("Advanced equalizer")).toBeNull());
    expect(r.container.querySelector(".eq")).not.toBeNull();
    expect(navLabels(r.container)).toContain("Sound");
    r.unmount();
  });
});
