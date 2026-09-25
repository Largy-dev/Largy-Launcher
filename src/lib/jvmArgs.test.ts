import { describe, expect, it } from "vitest";

import { jvmArgIssues, parseJvmArgs } from "./jvmArgs";

describe("parseJvmArgs", () => {
  it("splits on whitespace", () => {
    expect(parseJvmArgs("-Xmx2G -Dfoo=bar")).toEqual(["-Xmx2G", "-Dfoo=bar"]);
  });

  it("returns an empty array for blank input", () => {
    expect(parseJvmArgs("")).toEqual([]);
    expect(parseJvmArgs("   ")).toEqual([]);
  });

  it("drops leading, trailing, and repeated whitespace", () => {
    expect(parseJvmArgs("  -Xmx2G   -Dfoo=bar  ")).toEqual(["-Xmx2G", "-Dfoo=bar"]);
  });
});

describe("jvmArgIssues", () => {
  it("is silent for sane arguments", () => {
    expect(jvmArgIssues(["-XX:+UseG1GC", "--add-opens", "java.base/java.lang=ALL-UNNAMED"], 21)).toEqual([]);
  });

  it("flags two garbage collectors", () => {
    const [issue] = jvmArgIssues(["-XX:+UseG1GC", "-XX:+UseZGC"]);
    expect(issue).toContain("-XX:+UseZGC");
  });

  it("flags ZGC below Minecraft 1.21 only", () => {
    expect(jvmArgIssues(["-XX:+UseZGC"], 20)).toHaveLength(1);
    expect(jvmArgIssues(["-XX:+UseZGC"], 21)).toEqual([]);
  });

  it("flags bare words and memory flags", () => {
    expect(jvmArgIssues(["java", "-Xmx4G"])).toHaveLength(2);
  });
});
