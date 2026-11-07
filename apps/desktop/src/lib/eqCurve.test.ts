import { describe, expect, it } from "vitest";
import {
  ADVANCED_BAND_LABELS,
  bestFitPreview,
  curveSaver,
  DEVICE_BAND_LABELS,
  fromRaw,
  PAGE_COUNT,
  pageOf,
  toRaw,
  visibleBands,
} from "./eqCurve";

describe("eqCurve helpers", () => {
  it("paging_bounds", () => {
    expect(PAGE_COUNT).toBe(3);
    expect(visibleBands(0)).toEqual([0, 1, 2, 3]);
    expect(visibleBands(1)).toEqual([4, 5, 6, 7]);
    // The last page shows 7–10 so four faders are always visible.
    expect(visibleBands(2)).toEqual([6, 7, 8, 9]);
    // Out of range pages clamp, never an empty page.
    expect(visibleBands(-1)).toEqual([0, 1, 2, 3]);
    expect(visibleBands(9)).toEqual([6, 7, 8, 9]);
    for (let p = -2; p < 6; p++) expect(visibleBands(p)).toHaveLength(4);
    expect([0, 1, 2, 3, 4, 5, 6, 7, 8, 9].map(pageOf)).toEqual([0, 0, 0, 0, 1, 1, 1, 1, 2, 2]);
    expect(pageOf(-3)).toBe(0);
    expect(pageOf(42)).toBe(2);
    // Every band is visible on its own page.
    for (let i = 0; i < 10; i++) expect(visibleBands(pageOf(i))).toContain(i);
  });

  it("labels", () => {
    expect(ADVANCED_BAND_LABELS).toEqual([
      "31 Hz",
      "62 Hz",
      "125 Hz",
      "250 Hz",
      "500 Hz",
      "1 kHz",
      "2 kHz",
      "4 kHz",
      "8 kHz",
      "16 kHz",
    ]);
    expect(DEVICE_BAND_LABELS).toEqual(["Clear Bass", "400 Hz", "1 kHz", "2.5 kHz", "6.3 kHz", "16 kHz"]);
  });

  it("raw scale is centred on ten", () => {
    expect([-10, 0, 10, 42, -42].map(toRaw)).toEqual([0, 10, 20, 20, 0]);
    expect([0, 10, 20, 255].map(fromRaw)).toEqual([-10, 0, 10, 10]);
  });

  // Same fixtures as crates/mdr-proto/src/eq.rs (best_fit_*).
  it("best_fit_parity_with_rust", () => {
    expect(bestFitPreview([0, 0, 0, 0, 0, 0, 0, 0, 0, 0])).toEqual([0, 0, 0, 0, 0, 0]);
    expect(bestFitPreview([6, 6, 6, 6, 0, 0, 0, 0, 0, 0])).toEqual([6, 2, 0, 0, 0, 0]);
    expect(bestFitPreview([0, 0, 0, 0, 0, 0, 0, 10, 10, 10])).toEqual([0, 0, 0, 3, 10, 10]);
    expect(bestFitPreview([0, 0, 0, 0, 0, 0, 10, 10, 10, 10])).toEqual([0, 0, 0, 10, 10, 10]);
    expect(bestFitPreview([0, 0, 0, 0, 0, 4, 0, -10, 0, -7])).toEqual([0, 0, 4, -3, -3, -7]);
    expect(bestFitPreview([8, 6, 4, 2, 0, -2, 0, 2, 4, 6])).toEqual([5, 1, -2, 1, 3, 6]);
    // Half away from zero.
    const cb = (c: number[]) => bestFitPreview([...c, 0, 0, 0, 0, 0, 0, 0, 0].slice(0, 10))[0];
    expect(cb([1, 1])).toBe(1);
    expect(cb([-1, -1])).toBe(-1);
    expect(cb([3, 3])).toBe(2);
    expect(cb([-3, -3])).toBe(-2);
    expect(cb([1])).toBe(0);
    expect(cb([-1])).toBe(0);
    expect(cb([3])).toBe(1);
    expect(cb([-3])).toBe(-1);
    expect(bestFitPreview([0, 0, 0, -6, 0, 0, 0, 0, 0, 0])[1]).toBe(-2);
    // Clamps.
    expect(bestFitPreview(Array(10).fill(10))).toEqual(Array(6).fill(10));
    expect(bestFitPreview(Array(10).fill(-10))).toEqual(Array(6).fill(-10));
    expect(bestFitPreview(Array(10).fill(100))).toEqual(Array(6).fill(10));
  });

  it("curve saver sends the latest curve, one at a time", async () => {
    const sent: number[][] = [];
    let release: () => void = () => {};
    const save = (bands: number[]) => {
      sent.push(bands);
      return new Promise<void>((done) => (release = done));
    };
    const s = curveSaver(save);
    const a = s.save([1]);
    s.save([2]);
    const c = s.save([3]);
    expect(sent).toEqual([[1]]);
    release();
    await a;
    await Promise.resolve();
    await Promise.resolve();
    expect(sent).toEqual([[1], [3]]);
    release();
    await c;
    await s.idle();
    expect(sent).toEqual([[1], [3]]);
  });
});
