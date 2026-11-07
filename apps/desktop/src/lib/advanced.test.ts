import { describe, expect, it } from "vitest";
import { afterMark, frameLine, hexGroups, isSetOpcode, parseHex, type FrameEntry } from "./advanced";

const entry = (t_ms: number, dir: FrameEntry["dir"], table: FrameEntry["table"], bytes: number[]): FrameEntry => ({
  t_ms,
  dir,
  table,
  bytes,
});

describe("hexGroups", () => {
  it("prints uppercase two-digit groups", () => {
    expect(hexGroups([0x3e, 0x0c, 0x00, 0x0a, 0xff])).toBe("3E 0C 00 0A FF");
    expect(hexGroups([])).toBe("");
  });
});

describe("frameLine", () => {
  it("formats time, direction, table and hex", () => {
    expect(frameLine(entry(2234, "Sent", "T1", [0x3e, 0x0c]), 1000)).toEqual({
      t: "+1.234 s",
      arrow: "→",
      table: "T1",
      hex: "3E 0C",
    });
    expect(frameLine(entry(1000, "Received", "T2", [0x3c]), 1000)).toEqual({
      t: "+0.000 s",
      arrow: "←",
      table: "T2",
      hex: "3C",
    });
  });

  it("shows -- for an ACK or fragment without a table", () => {
    expect(frameLine(entry(1500, "Received", null, [0x3e, 0x01]), 1000).table).toBe("--");
  });
});

describe("isSetOpcode", () => {
  const T1_GETS = [
    0x00, 0x02, 0x04, 0x06, 0x10, 0x12, 0x20, 0x22, 0x26, 0x50, 0x56, 0x5a, 0x60, 0x66, 0xa0, 0xa2, 0xa6, 0xd0, 0xd2,
    0xd6, 0xe0, 0xe2, 0xe6, 0xf0, 0xf2, 0xf6, 0xfa,
  ];
  const T2_GETS = [
    0x06, 0x20, 0x22, 0x26, 0x30, 0x32, 0x36, 0x40, 0x42, 0x46, 0x56, 0x5a, 0x60, 0x62, 0x66, 0x70, 0x72, 0x76, 0xf0,
    0xf2, 0xf6, 0xfa,
  ];

  it("lets GETs through without confirmation", () => {
    for (const op of T1_GETS) expect(isSetOpcode("T1", [op, 0x01]), `T1 ${op}`).toBe(false);
    for (const op of T2_GETS) expect(isSetOpcode("T2", [op, 0x01]), `T2 ${op}`).toBe(false);
  });

  it("asks for confirmation on every other opcode", () => {
    for (const op of [0x58, 0x68, 0xf8, 0x01, 0x15, 0x98, 0xc4, 0x3c]) expect(isSetOpcode("T1", [op])).toBe(true);
    for (const op of [0x48, 0x50, 0x51, 0x3c, 0x07]) expect(isSetOpcode("T2", [op])).toBe(true);
    // A T1 GET that is not a T2 GET, and the other way round.
    expect(isSetOpcode("T2", [0x10])).toBe(true);
    expect(isSetOpcode("T1", [0x46])).toBe(true);
  });

  it("asks nothing for an empty payload", () => {
    expect(isSetOpcode("T1", [])).toBe(false);
  });
});

describe("parseHex", () => {
  it("reads pairs with or without separators", () => {
    expect(parseHex("58 03 01")).toEqual([0x58, 0x03, 0x01]);
    expect(parseHex("580301")).toEqual([0x58, 0x03, 0x01]);
    expect(parseHex(" 66:17-0a ")).toEqual([0x66, 0x17, 0x0a]);
  });

  it("refuses odd digits, non-hex and empty input", () => {
    expect(parseHex("583")).toBeNull();
    expect(parseHex("zz")).toBeNull();
    expect(parseHex("5 8")).toBeNull();
    expect(parseHex("")).toBeNull();
    expect(parseHex("   ")).toBeNull();
  });
});

describe("afterMark", () => {
  const a = entry(1, "Sent", "T1", [1]);
  const b = entry(2, "Received", "T1", [2]);
  const c = entry(3, "Sent", "T2", [3]);

  it("hides entries up to the mark (compared by value)", () => {
    expect(afterMark([a, b, c], { ...b, bytes: [2] })).toEqual([c]);
    expect(afterMark([a, b, c], null)).toEqual([a, b, c]);
  });

  it("shows everything once the mark has left the ring", () => {
    expect(afterMark([b, c], entry(0, "Sent", "T1", [9]))).toEqual([b, c]);
  });
});
