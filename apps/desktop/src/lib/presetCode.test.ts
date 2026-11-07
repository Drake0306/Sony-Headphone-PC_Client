import { describe, expect, it } from "vitest";
import { ALPHABET, crc10, decode, encodeAdvanced, encodeSound, normalise } from "./presetCode";

// Layout (README "Sharing a preset"): 15 Crockford base32 symbols × 5 bits.
//   symbol 0      kind: S (Sound) or A (Advanced), its alphabet value
//   symbols 1–12  60-bit payload, MSB first
//                 Sound: six raw bands 0..20 (5 bits each), ULT 0..2 (2 bits),
//                        slot (2 bits: 0 Manual, 1 Custom 1, 2 Custom 2, 3 none), zero padding
//                 Advanced: ten bands −10..10 stored +10 (5 bits each), zero padding
//   symbols 13–14 CRC-10 (poly 0x233, init 0, no reflection) over symbols 0–12 (65 bits)
// The two vectors below were computed once from this layout; they pin the format.
const SOUND_VECTOR = { bands: [15, 11, 8, 11, 13, 16], ult: 1 as const, slot: 1 as const };
const SOUND_CODE = "SFB8B-DGA00-000W7";
const ADVANCED_VECTOR = [8, 6, 4, 2, 0, -2, 0, 2, 4, -10];
const ADVANCED_CODE = "AJGEC-A8ACE-000ND";

const NOT_OURS = "That code is not a Hairline preset.";
const TYPO = "That code has a typo (check failed).";

describe("preset share codes", () => {
  it("uses the Crockford alphabet", () => {
    expect(ALPHABET).toBe("0123456789ABCDEFGHJKMNPQRSTVWXYZ");
    expect(ALPHABET).toHaveLength(32);
  });

  it("code_fixed_vectors", () => {
    expect(encodeSound(SOUND_VECTOR)).toBe(SOUND_CODE);
    expect(encodeAdvanced(ADVANCED_VECTOR)).toBe(ADVANCED_CODE);
    expect(decode(SOUND_CODE)).toEqual({ kind: "sound", ...SOUND_VECTOR });
    expect(decode(ADVANCED_CODE)).toEqual({ kind: "advanced", bands: ADVANCED_VECTOR });
  });

  it("code_is_formatted_in_three_groups", () => {
    const code = encodeSound({ bands: [10, 10, 10, 10, 10, 10], ult: 0, slot: 3 });
    expect(code).toMatch(/^S[0-9A-HJKMNP-TV-Z]{4}-[0-9A-HJKMNP-TV-Z]{5}-[0-9A-HJKMNP-TV-Z]{5}$/);
    expect(encodeAdvanced(Array(10).fill(0))[0]).toBe("A");
  });

  it("code_round_trips", () => {
    for (const ult of [0, 1, 2] as const) {
      for (const slot of [0, 1, 2, 3] as const) {
        for (const bands of [
          [0, 0, 0, 0, 0, 0],
          [20, 20, 20, 20, 20, 20],
          [0, 20, 1, 19, 10, 7],
        ]) {
          expect(decode(encodeSound({ bands, ult, slot }))).toEqual({ kind: "sound", bands, ult, slot });
        }
      }
    }
    for (const bands of [Array(10).fill(-10), Array(10).fill(10), [-10, 10, -1, 1, 0, 9, -9, 5, -5, 3]]) {
      expect(decode(encodeAdvanced(bands))).toEqual({ kind: "advanced", bands });
    }
  });

  it("code_rejects_typos_and_swaps", () => {
    const codes = [
      SOUND_CODE,
      ADVANCED_CODE,
      encodeSound({ bands: [0, 20, 3, 17, 10, 5], ult: 2, slot: 0 }),
      encodeAdvanced([-10, 10, -3, 4, 0, 7, -7, 2, 9, -1]),
    ];
    for (const code of codes) {
      const plain = normalise(code);
      for (let i = 0; i < plain.length; i++) {
        for (const c of ALPHABET) {
          if (c === plain[i]) continue;
          const bad = plain.slice(0, i) + c + plain.slice(i + 1);
          expect(decode(bad), `${bad} (position ${i})`).toHaveProperty("error");
        }
      }
      for (let i = 0; i + 1 < plain.length; i++) {
        if (plain[i] === plain[i + 1]) continue;
        const bad = plain.slice(0, i) + plain[i + 1] + plain[i] + plain.slice(i + 2);
        expect(decode(bad), `${bad} (swap ${i})`).toHaveProperty("error");
      }
    }
    // A typo in the payload is a check failure, not a foreign code.
    const p = normalise(SOUND_CODE);
    expect(decode(p.slice(0, 5) + (p[5] === "1" ? "2" : "1") + p.slice(6))).toEqual({ error: TYPO });
  });

  it("code_normalises_input", () => {
    const plain = normalise(SOUND_CODE);
    expect(plain).toBe(SOUND_CODE.replaceAll("-", ""));
    expect(decode(SOUND_CODE.toLowerCase())).toEqual(decode(SOUND_CODE));
    expect(decode(plain)).toEqual(decode(SOUND_CODE));
    expect(decode(`  ${plain.slice(0, 5)} ${plain.slice(5, 10)}\t${plain.slice(10)}\n`)).toEqual(decode(SOUND_CODE));
    // I and L read as 1, O as 0, in either case.
    expect(normalise("ilo-ILO")).toBe("110110");
    const zeros = ADVANCED_CODE.replaceAll("0", "o");
    expect(decode(zeros)).toEqual(decode(ADVANCED_CODE));
    const withOnes = encodeAdvanced([-9, -9, -9, -9, -9, -9, -9, -9, -9, -9]);
    expect(decode(withOnes.replaceAll("1", "l"))).toEqual(decode(withOnes));
    expect(decode(withOnes.replaceAll("1", "I"))).toEqual(decode(withOnes));
  });

  it("code_rejects_foreign_text", () => {
    expect(decode("")).toEqual({ error: NOT_OURS });
    expect(decode(SOUND_CODE.slice(0, -1))).toEqual({ error: NOT_OURS });
    expect(decode(`${SOUND_CODE}0`)).toEqual({ error: NOT_OURS });
    expect(decode(`U${SOUND_CODE.slice(1)}`)).toEqual({ error: NOT_OURS });
    expect(decode(`${SOUND_CODE.slice(0, 3)}*${SOUND_CODE.slice(4)}`)).toEqual({ error: NOT_OURS });
    expect(decode("https://example.com/x")).toEqual({ error: NOT_OURS });
  });

  /** A code with a valid check around any 65 bits. */
  function forge(bits: number[]): string {
    const all = [...bits, ...crcBits(crc10(bits))];
    let out = "";
    for (let i = 0; i < 75; i += 5) out += ALPHABET[parseInt(all.slice(i, i + 5).join(""), 2)];
    return out;
  }
  const crcBits = (crc: number) => Array.from({ length: 10 }, (_, i) => (crc >> (9 - i)) & 1);
  const toBits = (v: number, n: number) => Array.from({ length: n }, (_, i) => (v >> (n - 1 - i)) & 1);

  it("code_rejects_out_of_range_fields", () => {
    const sound = (bands: number[], ult: number, slot: number, pad = 0) => [
      ...toBits(ALPHABET.indexOf("S"), 5),
      ...bands.flatMap((b) => toBits(b, 5)),
      ...toBits(ult, 2),
      ...toBits(slot, 2),
      ...toBits(pad, 26),
    ];
    expect(decode(forge(sound([10, 10, 10, 10, 10, 10], 1, 3)))).toEqual({
      kind: "sound",
      bands: [10, 10, 10, 10, 10, 10],
      ult: 1,
      slot: 3,
    });
    expect(decode(forge(sound([10, 21, 10, 10, 10, 10], 0, 3)))).toEqual({ error: NOT_OURS });
    expect(decode(forge(sound([10, 10, 10, 10, 10, 10], 3, 3)))).toEqual({ error: NOT_OURS });
    expect(decode(forge(sound([10, 10, 10, 10, 10, 10], 0, 3, 1)))).toEqual({ error: NOT_OURS });
    const adv = (raw: number[]) => [...toBits(ALPHABET.indexOf("A"), 5), ...raw.flatMap((b) => toBits(b, 5)), ...toBits(0, 10)];
    expect(decode(forge(adv([0, 0, 0, 0, 0, 0, 0, 0, 0, 31])))).toEqual({ error: NOT_OURS });
    // Unknown kinds.
    for (const kind of ["0", "B", "Z"]) {
      expect(decode(forge([...toBits(ALPHABET.indexOf(kind), 5), ...Array(60).fill(0)]))).toEqual({ error: NOT_OURS });
    }
  });

  it("encode_refuses_bad_values", () => {
    expect(() => encodeSound({ bands: [10, 10, 10, 10, 10], ult: 0, slot: 3 })).toThrow();
    expect(() => encodeSound({ bands: [10, 10, 10, 10, 10, 21], ult: 0, slot: 3 })).toThrow();
    expect(() => encodeAdvanced([0, 0, 0, 0, 0, 0, 0, 0, 0, 11])).toThrow();
    expect(() => encodeAdvanced([0, 0, 0])).toThrow();
  });

  it("crc10 is the plain MSB-first CRC with poly 0x233", () => {
    expect(crc10([])).toBe(0);
    expect(crc10([1])).toBe(0x233);
    expect(crc10([1, 0])).toBe(((0x233 << 1) & 0x3ff) ^ 0x233);
  });
});
