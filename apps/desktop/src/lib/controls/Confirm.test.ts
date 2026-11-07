import { describe, expect, it, vi } from "vitest";
import { fireEvent, render } from "@testing-library/svelte";
import Confirm from "./Confirm.svelte";

const props = () => ({
  title: "Title",
  body: "Body",
  details: [{ label: "A", value: "+1" }],
  confirmLabel: "Send",
  onconfirm: vi.fn(),
  oncancel: vi.fn(),
});

describe("Confirm", () => {
  it("tab_wraps_inside_the_dialog", async () => {
    const r = render(Confirm, props());
    const cancel = r.getByRole("button", { name: "Cancel" });
    const send = r.getByRole("button", { name: "Send" });
    expect(document.activeElement).toBe(cancel);
    await fireEvent.keyDown(cancel, { key: "Tab" });
    expect(document.activeElement).toBe(send);
    await fireEvent.keyDown(send, { key: "Tab" });
    expect(document.activeElement).toBe(cancel);
    await fireEvent.keyDown(cancel, { key: "Tab", shiftKey: true });
    expect(document.activeElement).toBe(send);
    await fireEvent.keyDown(send, { key: "Tab", shiftKey: true });
    expect(document.activeElement).toBe(cancel);
    r.unmount();
  });

  it("restores_focus_on_close", () => {
    const opener = document.createElement("button");
    document.body.appendChild(opener);
    opener.focus();
    const r = render(Confirm, props());
    expect(document.activeElement).toBe(r.getByRole("button", { name: "Cancel" }));
    r.unmount();
    expect(document.activeElement).toBe(opener);
    opener.remove();
  });
});
