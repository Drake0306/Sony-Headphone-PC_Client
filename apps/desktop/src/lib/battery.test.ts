import { describe, expect, it } from "vitest";
import { historyNote, timeLeft } from "./battery";

describe("battery", () => {
  it("time_left_reads_hours_and_minutes", () => {
    expect(timeLeft(320)).toBe("about 5 h 20 m left");
    expect(timeLeft(40)).toBe("about 40 m left");
    expect(timeLeft(300)).toBe("about 5 h left");
    expect(timeLeft(null)).toBe("");
    expect(timeLeft(0)).toBe("");
  });

  it("history_note_counts_samples_since_the_first", () => {
    expect(historyNote({ minutes_left: null, samples: 0, since: null })).toBe("No samples yet");
    const since = new Date(2026, 10, 7, 9, 30).getTime() / 1000;
    const note = historyNote({ minutes_left: null, samples: 42, since });
    expect(note).toBe(`42 samples since ${new Date(since * 1000).toLocaleDateString()}`);
    expect(historyNote({ minutes_left: null, samples: 1, since })).toMatch(/^1 sample since /);
  });
});
