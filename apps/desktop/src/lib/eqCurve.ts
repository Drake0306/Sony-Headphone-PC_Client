// The Advanced equalizer's ten-band curve on the frontend: band names, paging
// (four faders at a time), the best fit onto the device's six values (a mirror
// of `mdr_proto::eq::best_fit`, so the fitted numbers follow a drag before the
// app has saved anything) and a saver that writes one curve at a time.

/** Centre frequencies of the ten bands, as `ADVANCED_BANDS_HZ` in mdr-proto. */
export const ADVANCED_BANDS_HZ = [31, 62, 125, 250, 500, 1000, 2000, 4000, 8000, 16000] as const;

export const ADVANCED_BAND_LABELS: readonly string[] = ADVANCED_BANDS_HZ.map((hz) =>
  hz >= 1000 ? `${hz / 1000} kHz` : `${hz} Hz`,
);

/** The device's six values in wire order (`DEVICE_BAND_LABELS` in mdr-proto). */
export const DEVICE_BAND_LABELS: readonly string[] = ["Clear Bass", "400 Hz", "1 kHz", "2.5 kHz", "6.3 kHz", "16 kHz"];

/** Lowest and highest level of a band, in either curve (21 steps). */
export const LEVEL_MIN = -10;
export const LEVEL_MAX = 10;

export const BAND_COUNT = ADVANCED_BANDS_HZ.length;
export const PAGE_SIZE = 4;
/** Pages of four; the last one is right-aligned (bands 7–10), never short. */
export const PAGE_COUNT = Math.ceil(BAND_COUNT / PAGE_SIZE);

const clampInt = (v: number, lo: number, hi: number) => Math.min(hi, Math.max(lo, Math.trunc(v)));

/** The band indexes shown on `page` (clamped to the pages there are). */
export function visibleBands(page: number): number[] {
  const p = clampInt(page, 0, PAGE_COUNT - 1);
  const start = Math.min(p * PAGE_SIZE, BAND_COUNT - PAGE_SIZE);
  return Array.from({ length: PAGE_SIZE }, (_, k) => start + k);
}

/** The page that shows band `index` (clamped to the bands there are). */
export function pageOf(index: number): number {
  return Math.min(PAGE_COUNT - 1, Math.floor(clampInt(index, 0, BAND_COUNT - 1) / PAGE_SIZE));
}

/** A level as the device's raw 0..=20 scale (10 is flat), clamped. */
export const toRaw = (v: number) => clampInt(v, LEVEL_MIN, LEVEL_MAX) - LEVEL_MIN;

/** A raw 0..=20 value as a level −10..=10, clamped. */
export const fromRaw = (r: number) => clampInt(r, 0, LEVEL_MAX - LEVEL_MIN) + LEVEL_MIN;

/** Rounded half away from zero (as Rust's `f64::round`), then clamped. */
function level(v: number): number {
  const r = Math.sign(v) * Math.round(Math.abs(v));
  return Math.min(LEVEL_MAX, Math.max(LEVEL_MIN, r)) + 0;
}

/** The value at `hz` on the line from (loHz, lo) to (hiHz, hi) in log frequency. */
const interpolate = (loHz: number, lo: number, hiHz: number, hi: number, hz: number) =>
  lo + (Math.log(hz / loHz) / Math.log(hiHz / loHz)) * (hi - lo);

/**
 * The closest six device values for a ten-band curve, exactly as
 * `mdr_proto::eq::best_fit`: Clear Bass is the mean of 31..250 Hz, 400 Hz
 * interpolates 250↔500, 1 kHz is direct, 2.5 kHz interpolates 2k↔4k, 6.3 kHz
 * interpolates 4k↔8k, 16 kHz is direct; each rounded half away from zero and
 * clamped to −10..=10.
 */
export function bestFitPreview(curve: readonly number[]): number[] {
  const c = curve;
  return [
    level((c[0] + c[1] + c[2] + c[3]) / 4),
    level(interpolate(250, c[3], 500, c[4], 400)),
    level(c[5]),
    level(interpolate(2000, c[6], 4000, c[7], 2500)),
    level(interpolate(4000, c[7], 8000, c[8], 6300)),
    level(c[9]),
  ];
}

export interface CurveSaver {
  /** Saves `bands`; resolves once it (or a later curve that replaced it) is saved. */
  save(bands: number[]): Promise<void>;
  /** Resolves when nothing is waiting to be saved. */
  idle(): Promise<void>;
}

/**
 * Writes curves one at a time through `write`; curves asked for while a write
 * runs collapse into the latest, so the store always ends on the last edit.
 */
export function curveSaver(write: (bands: number[]) => Promise<unknown>): CurveSaver {
  let running: Promise<void> | null = null;
  let next: number[] | null = null;
  let waiters: { resolve: () => void; reject: (e: unknown) => void }[] = [];

  async function pump() {
    while (next !== null) {
      const bands = next;
      const mine = waiters;
      next = null;
      waiters = [];
      try {
        await write(bands);
        mine.forEach((w) => w.resolve());
      } catch (e) {
        mine.forEach((w) => w.reject(e));
      }
    }
    running = null;
  }

  return {
    save(bands) {
      next = [...bands];
      const done = new Promise<void>((resolve, reject) => waiters.push({ resolve, reject }));
      running ??= pump();
      return done;
    },
    idle: () => running ?? Promise.resolve(),
  };
}
