import { describe, expect, it } from "vitest";
import { bluetoothLabel, hostPlaying, statusLabel, withFake } from "./state";

describe("status label", () => {
  it("status_label_names_connection_and_firmware", () => {
    expect(statusLabel(true, "1.2.0", false)).toBe("Connected · FW 1.2.0");
    expect(statusLabel(true, null, false)).toBe("Connected");
    expect(statusLabel(false, "1.2.0", false)).toBe("Not connected");
  });

  it("status_label_marks_fake_runs", () => {
    expect(statusLabel(true, "1.2.0", true)).toBe("FAKE · Connected · FW 1.2.0");
    expect(statusLabel(false, null, true)).toBe("FAKE · Not connected");
  });

  it("with_fake_prefixes_only_fake_runs", () => {
    expect(withFake("Black · LDAC", false)).toBe("Black · LDAC");
    expect(withFake("Black · LDAC", true)).toBe("FAKE · Black · LDAC");
    expect(withFake("", true)).toBe("FAKE");
  });
});

describe("reported values", () => {
  it("bluetooth_label_shows_only_what_the_transport_reported", () => {
    expect(bluetoothLabel(18, null)).toBe("channel 18");
    expect(bluetoothLabel(18, "A8:E6:E8:FE:D6:54")).toBe("channel 18 · A8:E6:E8:FE:D6:54");
    expect(bluetoothLabel(null, "A8:E6:E8:FE:D6:54")).toBe("A8:E6:E8:FE:D6:54");
    expect(bluetoothLabel(null, null)).toBe("--");
  });

  it("host_playing_follows_the_link_list", () => {
    expect(hostPlaying([])).toBe(false);
    expect(hostPlaying([{ name: "This Mac", playing: false }])).toBe(false);
    expect(hostPlaying([{ name: "This Mac", playing: true }])).toBe(true);
  });
});
