import { describe, expect, it } from "vitest";
import { statusLabel, withFake } from "./state";

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
