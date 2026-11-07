import { describe, expect, it } from "vitest";
import {
  afterMark,
  canRunProbe,
  frameLine,
  hexGroups,
  isSetOpcode,
  parseHex,
  probeChip,
  probeEstimate,
  probeRow,
  type FrameEntry,
  type ProbeResult,
} from "./advanced";

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
    // Separators are ignored before pairing, as the backend does.
    expect(parseHex("5 8")).toEqual([0x58]);
  });

  it("refuses odd digits, non-hex and empty input", () => {
    expect(parseHex("583")).toBeNull();
    expect(parseHex("zz")).toBeNull();
    expect(parseHex("58\t03")).toBeNull();
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

const result = (over: Partial<ProbeResult>): ProbeResult => ({
  table: "T1",
  opcode: 0xf6,
  inquired: 0x04,
  name: "SYSTEM_GET_PARAM",
  function: 0xf4,
  function_name: "Voice assistant settings",
  reply: [0xf7, 0x04, 0xff],
  declared: true,
  proven: false,
  ...over,
});

describe("probe helpers", () => {
  it("probeChip: silent, declared, proven, or nothing", () => {
    expect(probeChip(result({ reply: null }))).toBe("SILENT");
    expect(probeChip(result({ reply: null, declared: true }))).toBe("SILENT");
    expect(probeChip(result({}))).toBe("DECLARED");
    expect(probeChip(result({ declared: false, proven: true }))).toBe("PROVEN");
    // Answered, but it maps to no function: no chip.
    expect(probeChip(result({ declared: false, proven: false, function: null }))).toBeNull();
  });

  it("probeRow: table, opcode:inquired, name, reply hex", () => {
    expect(probeRow(result({}))).toEqual({
      key: "T1 F6:04",
      name: "Voice assistant settings",
      chip: "DECLARED",
      reply: "F7 04 FF",
    });
    const t2 = probeRow(
      result({ table: "T2", opcode: 0x32, inquired: 0x02, function_name: null, reply: null, declared: false }),
    );
    expect(t2).toEqual({ key: "T2 32:02", name: "SYSTEM_GET_PARAM", chip: "SILENT", reply: "" });
    expect(probeRow(result({ inquired: null })).key).toBe("T1 F6");
  });

  it("canRunProbe: connected, RUN typed exactly, not already running", () => {
    expect(canRunProbe(true, "RUN", "Idle")).toBe(true);
    expect(canRunProbe(true, "RUN", "Done")).toBe(true);
    expect(canRunProbe(false, "RUN", "Idle")).toBe(false);
    expect(canRunProbe(true, "run", "Idle")).toBe(false);
    expect(canRunProbe(true, " RUN", "Idle")).toBe(false);
    expect(canRunProbe(true, "RUN", "Running")).toBe(false);
  });
});

describe("probeEstimate", () => {
  it("counts the requests and rounds the worst case up to whole minutes", () => {
    expect(probeEstimate(441, 1500, 150)).toBe("441 requests, up to about 13 minutes");
    expect(probeEstimate(10, 1500, 150)).toBe("10 requests, up to about 1 minute");
    expect(probeEstimate(1, 1500, 150)).toBe("1 request, up to about 1 minute");
    expect(probeEstimate(0, 1500, 150)).toBeNull();
  });
});
