// The appearance themes: the list the Settings picker shows, and how a theme
// is applied to the document. The colours live in styles/tokens.css, one
// `[data-theme="<id>"]` block per id here; a test keeps the two in step.

import type { Theme } from "./state";

export interface ThemeOption {
  value: Theme;
  label: string;
  /** Light or dark paper; System follows the OS and shows both. */
  kind: "light" | "dark" | "system";
}

/** Every theme, in picker order: System, the light ones, then the dark ones. */
export const THEMES: readonly ThemeOption[] = [
  { value: "system", label: "System", kind: "system" },
  { value: "paper", label: "Paper", kind: "light" },
  { value: "latte", label: "Latte", kind: "light" },
  { value: "solarized", label: "Solarized", kind: "light" },
  { value: "dawn", label: "Dawn", kind: "light" },
  { value: "sepia", label: "Sepia", kind: "light" },
  { value: "kraft", label: "Kraft", kind: "light" },
  { value: "ink", label: "Ink", kind: "dark" },
  { value: "nord", label: "Nord", kind: "dark" },
  { value: "mocha", label: "Mocha", kind: "dark" },
  { value: "gruvbox", label: "Gruvbox", kind: "dark" },
  { value: "tokyo", label: "Tokyo Night", kind: "dark" },
  { value: "oled", label: "OLED Black", kind: "dark" },
];

/** True for an id the picker knows (a settings file from a newer build may name another). */
export function isTheme(value: string): value is Theme {
  return THEMES.some((t) => t.value === value);
}

/** Sets `data-theme` on `<html>`; System removes it so `prefers-color-scheme` decides. */
export function applyTheme(t: Theme) {
  const root = document.documentElement;
  if (t === "system" || !isTheme(t)) delete root.dataset.theme;
  else root.dataset.theme = t;
}
