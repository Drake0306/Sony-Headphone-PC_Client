// When the page itself breaks, say so on the page and in the diagnostics
// log, instead of leaving a white window. Plain DOM, no Svelte: it must work
// when the app failed to mount.

import { invoke } from "@tauri-apps/api/core";

/** The error as one readable paragraph: name, message and the first stack lines. */
export function describe(error: unknown): string {
  if (error instanceof Error) {
    const stack = (error.stack ?? "").split("\n").slice(0, 6).join("\n");
    return stack.includes(error.message) ? stack : `${error.name}: ${error.message}\n${stack}`.trim();
  }
  if (typeof error === "string") return error;
  try {
    return JSON.stringify(error);
  } catch {
    return String(error);
  }
}

/** Draws (or updates) the crash notice over the page. */
export function showCrash(message: string): HTMLElement {
  let box = document.querySelector<HTMLElement>(".crash");
  if (!box) {
    box = document.createElement("div");
    box.className = "crash";
    box.setAttribute("role", "alert");
    document.body.append(box);
  }
  box.textContent = "";
  const head = document.createElement("h1");
  head.textContent = "SHDC could not draw this window";
  const body = document.createElement("pre");
  body.textContent = message;
  const foot = document.createElement("p");
  foot.textContent = "The same text is in shdc.log in the app's log folder. Quit SHDC from the menu bar or tray and start it again.";
  box.append(head, body, foot);
  return box;
}

/** Shows the crash notice and sends the text to the diagnostics log. */
export function report(error: unknown): void {
  const message = describe(error);
  showCrash(message);
  invoke("report_frontend_error", { message }).catch(() => {});
}

/** Catches uncaught errors for the life of the page; rejections only go to the log. */
export function installCrashOverlay(): void {
  window.addEventListener("error", (e) => report(e.error ?? e.message));
  window.addEventListener("unhandledrejection", (e) => {
    invoke("report_frontend_error", { message: `unhandled rejection: ${describe(e.reason)}` }).catch(() => {});
  });
}
