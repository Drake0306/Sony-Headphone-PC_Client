import { afterEach, describe, expect, it, vi } from "vitest";

const invoke = vi.hoisted(() => vi.fn(async () => {}));
vi.mock("@tauri-apps/api/core", () => ({ invoke }));

import { describe as describeError, installCrashOverlay, report, showCrash } from "./crash";

afterEach(() => {
  document.body.innerHTML = "";
  invoke.mockClear();
});

describe("crash notice", () => {
  it("describes errors, strings and other values", () => {
    expect(describeError(new TypeError("x is not a function"))).toContain("TypeError: x is not a function");
    expect(describeError("plain")).toBe("plain");
    expect(describeError({ code: 7 })).toBe('{"code":7}');
  });

  it("draws one notice with the message and reuses it", () => {
    showCrash("first");
    const box = showCrash("second");
    expect(document.querySelectorAll(".crash").length).toBe(1);
    expect(box.getAttribute("role")).toBe("alert");
    expect(box.textContent).toContain("SHDC could not draw this window");
    expect(box.textContent).toContain("second");
    expect(box.textContent).not.toContain("first");
    expect(box.textContent).toContain("shdc.log");
  });

  it("reports an uncaught error on the page and to the log", () => {
    installCrashOverlay();
    window.dispatchEvent(new ErrorEvent("error", { error: new Error("boom at start"), message: "boom at start" }));
    expect(document.querySelector(".crash")?.textContent).toContain("boom at start");
    expect(invoke).toHaveBeenCalledWith("report_frontend_error", { message: expect.stringContaining("boom at start") });
  });

  it("logs an unhandled rejection without covering the page", () => {
    installCrashOverlay();
    const event = new Event("unhandledrejection") as Event & { reason: unknown };
    event.reason = new Error("late failure");
    window.dispatchEvent(event);
    expect(document.querySelector(".crash")).toBeNull();
    expect(invoke).toHaveBeenCalledWith("report_frontend_error", { message: expect.stringContaining("late failure") });
  });

  it("report() draws and logs", () => {
    report(new Error("mount failed"));
    expect(document.querySelector(".crash")?.textContent).toContain("mount failed");
    expect(invoke).toHaveBeenCalledTimes(1);
  });
});
