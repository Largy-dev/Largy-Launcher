import { describe, expect, it } from "vitest";

import { formatBytes, formatClock, formatDuration, formatEta, formatGb, formatRelative } from "./format";

describe("formatDuration", () => {
  it("formats hours, minutes and seconds compactly", () => {
    expect(formatDuration(45)).toBe("45 s");
    expect(formatDuration(12 * 60)).toBe("12 min");
    expect(formatDuration(3600 * 42 + 600)).toBe("42 h 10 min");
    expect(formatDuration(7200)).toBe("2 h");
  });

  it("clamps negatives to zero", () => {
    expect(formatDuration(-5)).toBe("0 s");
  });
});

describe("formatClock", () => {
  it("pads every segment", () => {
    expect(formatClock(3909)).toBe("01:05:09");
  });
});

describe("formatRelative", () => {
  const now = 1_700_000_000_000;
  const at = (secondsAgo: number) => now / 1000 - secondsAgo;

  it("walks through minutes, hours, yesterday and days", () => {
    expect(formatRelative(at(10), now)).toBe("à l'instant");
    expect(formatRelative(at(5 * 60), now)).toBe("il y a 5 min");
    expect(formatRelative(at(3 * 3600), now)).toBe("il y a 3 h");
    expect(formatRelative(at(86400 + 100), now)).toBe("hier");
    expect(formatRelative(at(3 * 86400), now)).toBe("il y a 3 j");
  });
});

describe("formatBytes / formatGb", () => {
  it("uses French units and decimal comma", () => {
    expect(formatBytes(512)).toBe("512 o");
    expect(formatBytes(1536 * 1024)).toBe("1,5 Mo");
    expect(formatGb(6144)).toBe("6 Go");
    expect(formatGb(5632)).toBe("5,5 Go");
  });
});

describe("formatEta", () => {
  it("returns null when no speed is known yet", () => {
    expect(formatEta(1000, 0)).toBeNull();
  });

  it("estimates the remaining time", () => {
    expect(formatEta(10_000, 1000)).toBe("10 s");
    expect(formatEta(600_000, 1000)).toBe("10 min");
  });
});
