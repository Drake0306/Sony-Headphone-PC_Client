import { describe, expect, it, vi } from "vitest";
import { fireEvent, render } from "@testing-library/svelte";
import { encodeSound } from "../presetCode";
import { qrModules } from "../qr";
import ShareCode from "./ShareCode.svelte";
import shareSrc from "./ShareCode.svelte?raw";

const CODE = encodeSound({ bands: [15, 11, 8, 11, 13, 16], ult: 1, slot: 1 });

describe("Share a code", () => {
  it("shows the title, the QR of the code, the code and Copy", async () => {
    const writeText = vi.fn(async () => {});
    Object.defineProperty(navigator, "clipboard", { value: { writeText }, configurable: true });
    const onclose = vi.fn();
    const r = render(ShareCode, { title: "Share this preset", code: CODE, onclose });
    expect(r.getByRole("dialog")).toBeTruthy();
    expect(r.getByText("Share this preset")).toBeTruthy();
    expect(r.getByText(CODE)).toBeTruthy();
    expect(r.getByText("Anyone with Hairline can paste this code")).toBeTruthy();
    const svg = r.container.querySelector("svg.qr")!;
    expect(svg).not.toBeNull();
    const { size, dark } = qrModules(CODE);
    expect(svg.getAttribute("viewBox")).toBe(`0 0 ${size + 8} ${size + 8}`);
    expect(dark.length).toBeGreaterThan(0);
    await fireEvent.click(r.getByRole("button", { name: "Copy" }));
    expect(writeText).toHaveBeenCalledWith(CODE);
    expect(await r.findByText("Copied")).toBeTruthy();
    await fireEvent.click(r.getByRole("button", { name: "Close" }));
    expect(onclose).toHaveBeenCalled();
    r.unmount();
  });

  it("draws the QR in tokens", () => {
    expect(shareSrc).toMatch(/var\(--ink\)/);
    expect(shareSrc).toMatch(/var\(--paper\)/);
  });

  it("qr encodes the code text", () => {
    const a = qrModules(CODE);
    const b = qrModules(CODE.replace(/.$/, "0"));
    expect(a.size).toBe(21);
    expect(a.dark).not.toEqual(b.dark);
  });
});
