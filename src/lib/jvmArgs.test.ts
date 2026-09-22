import { describe, expect, it } from "vitest";

import { parseJvmArgs } from "./jvmArgs";

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
