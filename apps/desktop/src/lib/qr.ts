// The QR beside a share code: the code text itself (no URL), drawn by the
// MIT-licensed qrcode-generator package as dark modules for an inline SVG.
// Type number 0 picks the smallest symbol; level M survives some smudging.

import qrcode from "qrcode-generator";

/** Quiet zone around the symbol, in modules. */
export const QR_MARGIN = 4;

/** The symbol's size in modules and its dark modules as [row, col]. */
export function qrModules(text: string): { size: number; dark: [number, number][] } {
  const qr = qrcode(0, "M");
  // Upper-case letters, digits and dashes: the compact alphanumeric mode.
  qr.addData(text, /^[0-9A-Z $%*+\-./:]*$/.test(text) ? "Alphanumeric" : "Byte");
  qr.make();
  const size = qr.getModuleCount();
  const dark: [number, number][] = [];
  for (let r = 0; r < size; r++) for (let c = 0; c < size; c++) if (qr.isDark(r, c)) dark.push([r, c]);
  return { size, dark };
}

/** One SVG path for the dark modules, offset by the quiet zone. */
export const qrPath = (dark: readonly [number, number][]) =>
  dark.map(([r, c]) => `M${c + QR_MARGIN} ${r + QR_MARGIN}h1v1h-1z`).join("");
