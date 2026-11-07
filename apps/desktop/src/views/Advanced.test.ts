import { beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, waitFor } from "@testing-library/svelte";

// A stand-in for the backend's Advanced commands and events.
const backend = vi.hoisted(() => ({
  rows: [] as unknown[],
  handlers: {} as Record<string, (e: { payload: unknown }) => void>,
  // When set, listen("frames") waits for this before resolving.
  hold: null as Promise<void> | null,
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(async (cmd: string, args: { on?: boolean; confirm?: string; hex?: string }) => {
    if (cmd === "capability_list") return backend.rows;
    if (cmd === "watch_frames") return null;
    if (cmd === "record_frames") {
      const path = args.on ? "/captures/local/frames-1.log" : null;
      setTimeout(() => backend.handlers.recording?.({ payload: { path } }), 0);
      return path;
    }
    if (cmd === "send_raw") {
      if (args.hex?.startsWith("58") && args.confirm !== "SEND") throw "type SEND to send a SET";
      return null;
    }
  }),
}));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(async (name: string, h: (e: { payload: unknown }) => void) => {
    if (name === "frames" && backend.hold) await backend.hold;
    backend.handlers[name] = h;
    return () => delete backend.handlers[name];
  }),
  emitTo: vi.fn(async () => {}),
}));

import { invoke } from "@tauri-apps/api/core";
import { emptyState } from "../lib/state";
import Advanced from "./Advanced.svelte";

const calls = (cmd: string) => vi.mocked(invoke).mock.calls.filter(([c]) => c === cmd).map(([, a]) => a);
const renderIt = () => render(Advanced, { st: emptyState() });

describe("Advanced", () => {
  beforeEach(() => {
    backend.rows = [
      { table: "T1", id: 0x12, name: "Codec indicator", declared: true, proven: false },
      { table: "T1", id: 0x6b, name: null, declared: false, proven: true },
      { table: "T2", id: 0x50, name: "Safe listening (HBS 1)", declared: false, proven: false },
    ];
    backend.handlers = {};
    backend.hold = null;
    vi.mocked(invoke).mockClear();
  });

  it("renders the capability rows the backend lists", async () => {
    const r = renderIt();
    await waitFor(() => expect(r.getByText("Codec indicator")).toBeInTheDocument());
    const codec = r.getByText("Codec indicator").closest("div")!;
    expect(codec.textContent).toContain("12");
    expect(codec.textContent).toContain("DECLARED");
    expect(codec.textContent).not.toContain("PROVEN");
    const unnamed = r.getByText("6B").closest("div")!;
    expect(unnamed.textContent).toContain("--");
    expect(unnamed.textContent).toContain("PROVEN");
    const faded = r.getByText("Safe listening (HBS 1)").closest("div")!;
    expect(faded).toHaveClass("faded");
    expect(codec).not.toHaveClass("faded");
    expect(r.container.querySelectorAll(".caps > div")).toHaveLength(3);
  });

  it("asks for SEND only on a payload that is not a GET", async () => {
    const r = renderIt();
    const hex = r.getByLabelText("Raw bytes");
    await fireEvent.input(hex, { target: { value: "58 03 01" } });
    expect(r.getByPlaceholderText("type SEND")).toBeInTheDocument();
    await fireEvent.input(hex, { target: { value: "66 17" } });
    expect(r.queryByPlaceholderText("type SEND")).toBeNull();
  });

  it("shows the backend's refusal for a SET sent without SEND", async () => {
    const r = renderIt();
    await fireEvent.input(r.getByLabelText("Raw bytes"), { target: { value: "58 03 01" } });
    await fireEvent.click(r.getByText("Send", { selector: "button" }));
    await waitFor(() => expect(r.getByText("type SEND to send a SET")).toHaveClass("live"));
    expect(calls("send_raw").at(-1)).toEqual({ table: "t1", hex: "58 03 01", confirm: "" });
  });

  it("sends a confirmed SET and says Sent", async () => {
    const r = renderIt();
    await fireEvent.click(r.getByText("T2", { selector: "button" }));
    await fireEvent.input(r.getByLabelText("Raw bytes"), { target: { value: "58 03 01" } });
    await fireEvent.input(r.getByPlaceholderText("type SEND"), { target: { value: "SEND" } });
    await fireEvent.click(r.getByText("Send", { selector: "button" }));
    await waitFor(() => expect(r.getByText("Sent")).not.toHaveClass("live"));
    expect(calls("send_raw").at(-1)).toEqual({ table: "t2", hex: "58 03 01", confirm: "SEND" });
  });

  it("records and shows the file path while recording", async () => {
    const r = renderIt();
    await fireEvent.click(r.getByText("Record", { selector: "button" }));
    await waitFor(() => expect(r.getByText("/captures/local/frames-1.log")).toBeInTheDocument());
    expect(r.getByText("Stop", { selector: "button" })).toBeInTheDocument();
    await fireEvent.click(r.getByText("Stop", { selector: "button" }));
    await waitFor(() => expect(r.queryByText("/captures/local/frames-1.log")).toBeNull());
    expect(r.getByText("Record", { selector: "button" })).toBeInTheDocument();
    expect(calls("record_frames")).toEqual([{ on: true }, { on: false }]);
  });

  it("shows the frame snapshot and clears it locally", async () => {
    const r = renderIt();
    await waitFor(() => expect(backend.handlers.frames).toBeDefined());
    backend.handlers.frames({
      payload: [
        { t_ms: 1000, dir: "Sent", table: "T1", bytes: [0x3e, 0x0c, 0x00] },
        { t_ms: 1250, dir: "Received", table: null, bytes: [0x3e, 0x01, 0x3c] },
      ],
    });
    await waitFor(() => expect(r.getByText("3E 0C 00")).toBeInTheDocument());
    expect(r.getByText("3E 01 3C")).toBeInTheDocument();
    expect(r.getByText("+0.250 s")).toBeInTheDocument();
    await fireEvent.click(r.getByText("Clear", { selector: "button" }));
    expect(r.queryByText("3E 0C 00")).toBeNull();
    backend.handlers.frames({
      payload: [
        { t_ms: 1000, dir: "Sent", table: "T1", bytes: [0x3e, 0x0c, 0x00] },
        { t_ms: 1250, dir: "Received", table: null, bytes: [0x3e, 0x01, 0x3c] },
        { t_ms: 1300, dir: "Sent", table: "T2", bytes: [0x3e, 0x0e, 0x06] },
      ],
    });
    await waitFor(() => expect(r.getByText("3E 0E 06")).toBeInTheDocument());
    expect(r.queryByText("3E 01 3C")).toBeNull();
  });

  it("watches frames while mounted", async () => {
    const r = renderIt();
    await waitFor(() => expect(calls("watch_frames")).toEqual([{ on: true }]));
    r.unmount();
    expect(calls("watch_frames")).toEqual([{ on: true }, { on: false }]);
  });

  it("never starts watching once unmounted before listening", async () => {
    let release = () => {};
    backend.hold = new Promise<void>((done) => (release = done));
    const r = renderIt();
    r.unmount();
    release();
    await new Promise((done) => setTimeout(done, 10));
    expect(calls("watch_frames")).toEqual([{ on: false }]);
  });

  it("re-reads capabilities only when the link or the lists change", async () => {
    const st = emptyState();
    const r = render(Advanced, { st });
    await waitFor(() => expect(calls("capability_list")).toHaveLength(1));
    await r.rerender({ st: { ...st, volume: { value: 12, status: "Confirmed" } } });
    await new Promise((done) => setTimeout(done, 10));
    expect(calls("capability_list")).toHaveLength(1);
    await r.rerender({ st: { ...st, connected: true, model: "ULT WEAR" } });
    await waitFor(() => expect(calls("capability_list")).toHaveLength(2));
  });
});
