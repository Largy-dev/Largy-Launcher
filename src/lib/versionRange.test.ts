import { describe, expect, it } from "vitest";

import { compareVersions, satisfies } from "./versionRange";

describe("compareVersions", () => {
  it("compares numerically, segment by segment", () => {
    expect(compareVersions("1.10", "1.9")).toBeGreaterThan(0);
    expect(compareVersions("1.6.3", "1.6.3")).toBe(0);
    expect(compareVersions("10.7.19.85", "11")).toBeLessThan(0);
    expect(compareVersions("2.0", "2")).toBe(0);
  });
});

describe("satisfies — Maven ranges (Forge / NeoForge)", () => {
  it("handles open, closed and half-open bounds", () => {
    expect(satisfies("1.6.1", "(,1.6.2)", true)).toBe(true);
    expect(satisfies("1.6.3", "(,1.6.2)", true)).toBe(false);
    expect(satisfies("10.7.19", "[10.7.14,11-)", true)).toBe(true);
    expect(satisfies("11.0", "[10.7.14,11)", true)).toBe(false);
    expect(satisfies("1.2", "[1.2]", true)).toBe(true);
    expect(satisfies("1.3", "[1.0,1.2],[1.4,)", true)).toBe(false);
    expect(satisfies("1.5", "[1.0,1.2],[1.4,)", true)).toBe(true);
  });

  it("treats a bare version as that version or newer", () => {
    expect(satisfies("47.2.0", "47.1", true)).toBe(true);
    expect(satisfies("46.0", "47.1", true)).toBe(false);
  });
});

describe("satisfies — npm-style predicates (Fabric / Quilt)", () => {
  it("handles comparators, alternatives and wildcards", () => {
    expect(satisfies("0.4.9", "<0.5", false)).toBe(true);
    expect(satisfies("0.5.0", "<0.5", false)).toBe(false);
    expect(satisfies("2.0", "1.0 || 2.0", false)).toBe(true);
    expect(satisfies("1.2.7", "1.2.x", false)).toBe(true);
    expect(satisfies("1.3.0", "1.2.x", false)).toBe(false);
    expect(satisfies("1.5", ">=1.2 <2", false)).toBe(true);
    expect(satisfies("1.9.1", "^1.2", false)).toBe(true);
    expect(satisfies("2.0.0", "^1.2", false)).toBe(false);
    expect(satisfies("1.2.9", "~1.2", false)).toBe(true);
    expect(satisfies("1.3.0", "~1.2", false)).toBe(false);
  });

  it("never alarms on unknown versions or ranges it can't read", () => {
    expect(satisfies(null, "<1", false)).toBe(false);
    expect(satisfies("1.0", null, false)).toBe(true);
    expect(satisfies("1.0", "!!garbage", true)).toBe(false);
    expect(satisfies("1.0", "!!garbage", false)).toBe(false);
  });
});
