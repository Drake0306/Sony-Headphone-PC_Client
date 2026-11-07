// Equalizer share codes: a setting as 15 Crockford base32 symbols, shown as
// XXXXX-XXXXX-XXXXX. Each symbol carries 5 bits, 75 in all, MSB first:
//
//   symbol 0       kind, as its alphabet value: S (Sound) or A (Advanced)
//   symbols 1–12   60-bit payload
//                    Sound:    six raw bands 0..20 (5 bits each, device wire
//                              order), ULT 0..2 (2 bits), slot (2 bits:
//                              0 Manual, 1 Custom 1, 2 Custom 2, 3 none),
//                              then 26 zero bits
//                    Advanced: ten bands −10..10 stored as 0..20 (5 bits
//                              each, 31 Hz .. 16 kHz), then 10 zero bits
//   symbols 13–14  CRC-10 (polynomial 0x233, init 0, no reflection, no final
//                  xor) over the 65 bits of symbols 0–12
//
// A ten-bit CRC catches every error confined to ten consecutive bits, so any
// one wrong symbol and any swapped adjacent pair is rejected. Input is read
// leniently: whitespace and dashes are dropped, case is ignored, I and L read
// as 1 and O as 0. The same layout is in README.md ("Sharing a preset").

export const ALPHABET = "0123456789ABCDEFGHJKMNPQRSTVWXYZ";

export const CODE_LENGTH = 15;
const BITS = 5;
const PAYLOAD_SYMBOLS = 12;
const CRC_BITS = 10;
const POLY = 0x233;

const KIND_SOUND = ALPHABET.indexOf("S");
const KIND_ADVANCED = ALPHABET.indexOf("A");

const SOUND_BANDS = 6;
const ADVANCED_BANDS = 10;
const RAW_MAX = 20;
const LEVEL_MIN = -10;
const LEVEL_MAX = 10;

export const NOT_A_CODE = "That code is not a Hairline preset.";
export const BAD_CHECK = "That code has a typo (check failed).";

export type Ult = 0 | 1 | 2;
/** 0 Manual, 1 Custom 1, 2 Custom 2, 3 no slot (the receiver picks one). */
export type Slot = 0 | 1 | 2 | 3;

export interface SoundCode {
  kind: "sound";
  /** Raw 0..20 (10 is flat), device wire order. */
  bands: number[];
  ult: Ult;
  slot: Slot;
}

export interface AdvancedCode {
  kind: "advanced";
  /** Levels −10..10, 31 Hz .. 16 kHz. */
  bands: number[];
}

export type Decoded = SoundCode | AdvancedCode | { error: string };

/** The CRC-10 (poly 0x233, init 0, MSB first, no reflection) of a bit string. */
export function crc10(bits: readonly number[]): number {
  let reg = 0;
  for (const bit of bits) {
    const feedback = ((reg >> (CRC_BITS - 1)) & 1) ^ (bit & 1);
    reg = (reg << 1) & ((1 << CRC_BITS) - 1);
    if (feedback) reg ^= POLY;
  }
  return reg;
}

/** Drops whitespace and dashes, upper-cases, reads I and L as 1 and O as 0. */
export function normalise(text: string): string {
  return text
    .replace(/[\s-]+/g, "")
    .toUpperCase()
    .replace(/[IL]/g, "1")
    .replace(/O/g, "0");
}

const pushBits = (out: number[], value: number, n: number) => {
  for (let i = n - 1; i >= 0; i--) out.push((value >> i) & 1);
};

const readBits = (bits: readonly number[], at: number, n: number) => {
  let v = 0;
  for (let i = 0; i < n; i++) v = (v << 1) | bits[at + i];
  return v;
};

const isInt = (v: unknown, lo: number, hi: number): v is number =>
  typeof v === "number" && Number.isInteger(v) && v >= lo && v <= hi;

/** Kind and payload fields (65 bits) → the formatted code with its check. */
function finish(kind: number, payload: number[]): string {
  const bits: number[] = [];
  pushBits(bits, kind, BITS);
  bits.push(...payload);
  while (bits.length < BITS * (1 + PAYLOAD_SYMBOLS)) bits.push(0);
  pushBits(bits, crc10(bits), CRC_BITS);
  let plain = "";
  for (let i = 0; i < bits.length; i += BITS) plain += ALPHABET[readBits(bits, i, BITS)];
  return `${plain.slice(0, 5)}-${plain.slice(5, 10)}-${plain.slice(10)}`;
}

/** A Sound setting as a code; throws on values outside the layout. */
export function encodeSound({ bands, ult, slot }: { bands: readonly number[]; ult: number; slot: number }): string {
  if (bands.length !== SOUND_BANDS || !bands.every((b) => isInt(b, 0, RAW_MAX))) {
    throw new RangeError(`expected ${SOUND_BANDS} bands of 0..${RAW_MAX}`);
  }
  if (!isInt(ult, 0, 2)) throw new RangeError("ULT must be 0..2");
  if (!isInt(slot, 0, 3)) throw new RangeError("slot must be 0..3");
  const payload: number[] = [];
  for (const b of bands) pushBits(payload, b, BITS);
  pushBits(payload, ult, 2);
  pushBits(payload, slot, 2);
  return finish(KIND_SOUND, payload);
}

/** An Advanced curve as a code; throws on values outside the layout. */
export function encodeAdvanced(bands: readonly number[]): string {
  if (bands.length !== ADVANCED_BANDS || !bands.every((b) => isInt(b, LEVEL_MIN, LEVEL_MAX))) {
    throw new RangeError(`expected ${ADVANCED_BANDS} bands of ${LEVEL_MIN}..${LEVEL_MAX}`);
  }
  const payload: number[] = [];
  for (const b of bands) pushBits(payload, b - LEVEL_MIN, BITS);
  return finish(KIND_ADVANCED, payload);
}

/** The setting in `text`, or why it is not one. */
export function decode(text: string): Decoded {
  const plain = normalise(text);
  if (plain.length !== CODE_LENGTH) return { error: NOT_A_CODE };
  const bits: number[] = [];
  for (const c of plain) {
    const v = ALPHABET.indexOf(c);
    if (v < 0) return { error: NOT_A_CODE };
    pushBits(bits, v, BITS);
  }
  const body = bits.slice(0, BITS * (1 + PAYLOAD_SYMBOLS));
  if (crc10(body) !== readBits(bits, body.length, CRC_BITS)) return { error: BAD_CHECK };

  const kind = readBits(bits, 0, BITS);
  let at = BITS;
  const take = (n: number) => {
    const v = readBits(bits, at, n);
    at += n;
    return v;
  };
  const padded = () => body.slice(at).every((b) => b === 0);

  if (kind === KIND_SOUND) {
    const bands = Array.from({ length: SOUND_BANDS }, () => take(BITS));
    const ult = take(2);
    const slot = take(2) as Slot;
    if (!bands.every((b) => b <= RAW_MAX) || ult > 2 || !padded()) return { error: NOT_A_CODE };
    return { kind: "sound", bands, ult: ult as Ult, slot };
  }
  if (kind === KIND_ADVANCED) {
    const raw = Array.from({ length: ADVANCED_BANDS }, () => take(BITS));
    if (!raw.every((b) => b <= LEVEL_MAX - LEVEL_MIN) || !padded()) return { error: NOT_A_CODE };
    return { kind: "advanced", bands: raw.map((b) => b + LEVEL_MIN) };
  }
  return { error: NOT_A_CODE };
}

/** The preset id a slot names (Manual, Custom 1, Custom 2), or null for "no slot". */
export const slotPreset = (slot: Slot): number | null => (slot === 3 ? null : 0xa0 + slot);

/** The slot of a preset id: Manual 0, Custom 1 1, Custom 2 2, anything else 3. */
export const presetSlot = (preset: number | null): Slot =>
  preset === 0xa0 ? 0 : preset === 0xa1 ? 1 : preset === 0xa2 ? 2 : 3;
