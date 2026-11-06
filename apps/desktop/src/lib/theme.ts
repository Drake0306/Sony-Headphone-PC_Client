// Applies the Paper / Ink / System appearance to the document.

import type { Theme } from "./state";

/** Sets `data-theme` on `<html>`; System removes it so `prefers-color-scheme` decides. */
export function applyTheme(t: Theme) {
  const root = document.documentElement;
  if (t === "paper" || t === "ink") root.dataset.theme = t;
  else delete root.dataset.theme;
}
