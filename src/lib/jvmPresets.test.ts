import { describe, expect, it } from "vitest";

import { JVM_PRESETS, isPresetActive, togglePreset } from "./jvmPresets";

const utf8 = JVM_PRESETS.find((p) => p.id === "utf8")!;
const aikar = JVM_PRESETS.find((p) => p.id === "aikar")!;

describe("togglePreset", () => {
  it("adds a preset's flags after the user's own", () => {
    const next = togglePreset(["-Dfoo=bar"], utf8);
    expect(next).toEqual(["-Dfoo=bar", "-Dfile.encoding=UTF-8"]);
    expect(isPresetActive(next, utf8)).toBe(true);
  });

  it("removes only that preset's flags", () => {
    const withBoth = togglePreset(togglePreset(["-Dfoo=bar"], utf8), aikar);
    const next = togglePreset(withBoth, aikar);
    expect(next).toEqual(["-Dfoo=bar", "-Dfile.encoding=UTF-8"]);
  });

  it("does not duplicate a flag the user already had", () => {
    const next = togglePreset(["-XX:+UseG1GC"], aikar);
    expect(next.filter((a) => a === "-XX:+UseG1GC")).toHaveLength(1);
  });
});
