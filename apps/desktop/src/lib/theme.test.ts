import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";
import { applyTheme, isTheme, THEMES } from "./theme";

// Read as text (Vitest stubs CSS imports); Vitest runs from apps/desktop.
const TOKENS = readFileSync(resolve(process.cwd(), "src/styles/tokens.css"), "utf8");
const REQUIRED = ["--paper", "--ink", "--hair", "--muted", "--box", "--stripe", "--ult", "--on-ult", "color-scheme"];

/** The declarations inside `[data-theme="<id>"] { ... }`. */
function block(id: string): string {
  const m = TOKENS.match(new RegExp(`\\[data-theme="${id}"\\]\\s*\\{([^}]*)\\}`));
  if (!m) throw new Error(`tokens.css has no block for ${id}`);
  return m[1];
}

describe("themes", () => {
  it("lists nineteen distinct ids with System first", () => {
    const ids = THEMES.map((t) => t.value);
    expect(ids.length).toBe(19);
    expect(new Set(ids).size).toBe(ids.length);
    expect(ids[0]).toBe("system");
    expect(THEMES.filter((t) => t.kind === "light").length).toBe(10);
    expect(THEMES.filter((t) => t.kind === "dark").length).toBe(8);
  });

  it("every theme except System has a tokens.css block with the full token set", () => {
    for (const t of THEMES) {
      if (t.value === "system") continue;
      const css = block(t.value);
      for (const token of REQUIRED) expect(css, `${t.value} sets ${token}`).toContain(`${token}:`);
      expect(css).toContain(t.kind === "dark" ? "color-scheme: dark" : "color-scheme: light");
    }
  });

  it("a flair theme sets all sixteen flair tokens, a palette theme none, and the defaults block resets them", () => {
    const FLAIR = ["--wash", "--accent-2", "--accent-3", "--glow", ...[1, 2, 3, 4, 5, 6].flatMap((n) => [`--band-${n}`, `--fill-${n}`])];
    const defaults = TOKENS.match(/:root,\s*\[data-theme\] \{([^}]*)\}/);
    expect(defaults, "a defaults block for :root and every themed element").toBeTruthy();
    for (const token of FLAIR) expect(defaults![1], `defaults ${token}`).toContain(`${token}:`);
    for (const t of THEMES) {
      if (t.value === "system") continue;
      const css = block(t.value);
      const set = FLAIR.filter((token) => css.includes(`${token}:`)).length;
      expect([0, FLAIR.length], `${t.value} sets ${set} flair tokens`).toContain(set);
    }
    const flair = THEMES.filter((t) => t.value !== "system" && block(t.value).includes("--wash:")).map((t) => t.value);
    expect(flair.sort()).toEqual(["acid", "aurora", "riso", "spectrum", "sunset", "synthwave"]);
  });

  it("tokens.css names no theme the picker lacks", () => {
    const inCss = [...TOKENS.matchAll(/\[data-theme="([a-z]+)"\]/g)].map((m) => m[1]);
    for (const id of inCss) expect(isTheme(id), `picker knows ${id}`).toBe(true);
  });

  it("the Ink block and the System dark block carry the same values", () => {
    const ink = block("ink").replace(/\s+/g, " ").trim();
    const m = TOKENS.match(/:root:not\(\[data-theme\]\)\s*\{([^}]*)\}/);
    expect(m, "system dark block").toBeTruthy();
    expect(m![1].replace(/\s+/g, " ").trim()).toBe(ink);
  });

  it("applies a theme as data-theme and removes it for System or an unknown id", () => {
    applyTheme("nord");
    expect(document.documentElement.dataset.theme).toBe("nord");
    applyTheme("system");
    expect(document.documentElement.dataset.theme).toBeUndefined();
    applyTheme("kraft");
    applyTheme("someday" as never);
    expect(document.documentElement.dataset.theme).toBeUndefined();
  });
});
