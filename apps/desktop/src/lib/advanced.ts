// The Advanced screen's data: the capability rows and wire frames the backend
// sends (apps/desktop/src-tauri/src/{device_record,frame_ring}.rs), and the
// helpers that print them and decide when a raw command needs "SEND".

export type TableName = "T1" | "T2";

/** `CapabilityRow`: one declared or proven support function. */
export interface CapabilityRow {
  table: TableName;
  id: number;
  /** `null` when the id is not in the name table. */
  name: string | null;
  declared: boolean;
  proven: boolean;
}

/** `FrameEntry`: one whole wire frame (3E … 3C); `table` is null for ACKs and fragments. */
export interface FrameEntry {
  t_ms: number;
  dir: "Sent" | "Received";
  table: TableName | null;
  bytes: number[];
}

/** The "recording" event. */
export interface RecordingEvent {
  path: string | null;
}

const byte = (b: number) => b.toString(16).toUpperCase().padStart(2, "0");

/** "3E 0C 00 …": uppercase two-digit groups. */
export const hexGroups = (bytes: number[]) => bytes.map(byte).join(" ");

/** One frame-log line; `firstTms` is the time shown as +0. */
export function frameLine(entry: FrameEntry, firstTms: number) {
  return {
    t: `+${((entry.t_ms - firstTms) / 1000).toFixed(3)} s`,
    arrow: entry.dir === "Sent" ? "→" : "←",
    table: entry.table ?? "--",
    hex: hexGroups(entry.bytes),
  };
}

// The GET opcodes of mdr-proto's role tables (crates/mdr-proto/src/function.rs):
// a payload starting with anything else needs the typed confirmation, as the
// backend's `send_raw` requires.
const GETS: Record<TableName, ReadonlySet<number>> = {
  T1: new Set([
    0x00, 0x02, 0x04, 0x06, 0x10, 0x12, 0x20, 0x22, 0x26, 0x50, 0x56, 0x5a, 0x60, 0x66, 0xa0, 0xa2, 0xa6, 0xd0, 0xd2,
    0xd6, 0xe0, 0xe2, 0xe6, 0xf0, 0xf2, 0xf6, 0xfa,
  ]),
  T2: new Set([
    0x06, 0x20, 0x22, 0x26, 0x30, 0x32, 0x36, 0x40, 0x42, 0x46, 0x56, 0x5a, 0x60, 0x62, 0x66, 0x70, 0x72, 0x76, 0xf0,
    0xf2, 0xf6, 0xfa,
  ]),
};

/** True when the payload's opcode is not a GET (SET, RET, NTFY or unknown). */
export const isSetOpcode = (table: TableName, bytes: number[]) => bytes.length > 0 && !GETS[table].has(bytes[0]);

/** Hex pairs, optionally separated by spaces, ":" or "-"; null when malformed or empty. */
export function parseHex(text: string): number[] | null {
  const groups = text.trim().split(/[\s:-]+/).filter((g) => g !== "");
  if (groups.length === 0) return null;
  const out: number[] = [];
  for (const g of groups) {
    if (!/^(?:[0-9a-fA-F]{2})+$/.test(g)) return null;
    for (let i = 0; i < g.length; i += 2) out.push(parseInt(g.slice(i, i + 2), 16));
  }
  return out;
}

const same = (a: FrameEntry, b: FrameEntry) =>
  a.t_ms === b.t_ms &&
  a.dir === b.dir &&
  a.table === b.table &&
  a.bytes.length === b.bytes.length &&
  a.bytes.every((x, i) => x === b.bytes[i]);

/**
 * The entries after `mark` (the last entry when Clear was pressed). Once the
 * mark has left the 500-frame ring every entry is newer, so all are shown.
 */
export function afterMark(entries: FrameEntry[], mark: FrameEntry | null): FrameEntry[] {
  if (mark === null) return entries;
  for (let i = entries.length - 1; i >= 0; i--) if (same(entries[i], mark)) return entries.slice(i + 1);
  return entries;
}
