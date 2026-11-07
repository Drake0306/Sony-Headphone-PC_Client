import { describe, expect, it, vi } from "vitest";
import { render, waitFor } from "@testing-library/svelte";

const backend = vi.hoisted(() => ({
  estimate: { minutes_left: null as number | null, samples: 3, since: 1_000 },
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(async (cmd: string) => (cmd === "battery_estimate" ? backend.estimate : null)),
}));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(async () => () => {}),
  emitTo: vi.fn(async () => {}),
}));

import { invoke } from "@tauri-apps/api/core";
import { emptyState, type DeviceState } from "../lib/state";
import Overview from "./Overview.svelte";

const withBattery = (): DeviceState => ({
  ...emptyState(),
  connected: true,
  battery: { value: [62, "NotCharging"], status: "Confirmed" },
});

describe("Overview time left", () => {
  it("renders nothing without an estimate", async () => {
    backend.estimate = { minutes_left: null, samples: 3, since: 1_000 };
    const r = render(Overview, { st: withBattery() });
    await waitFor(() => expect(vi.mocked(invoke)).toHaveBeenCalledWith("battery_estimate"));
    await new Promise((done) => setTimeout(done, 10));
    expect(r.container.textContent).not.toMatch(/left/);
    expect(r.container.querySelector(".left")).toBeNull();
    r.unmount();
  });

  it("shows the estimate under the battery number", async () => {
    backend.estimate = { minutes_left: 320, samples: 30, since: 1_000 };
    const r = render(Overview, { st: withBattery() });
    await waitFor(() => expect(r.getByText("about 5 h 20 m left")).toBeInTheDocument());
    r.unmount();
  });
});
