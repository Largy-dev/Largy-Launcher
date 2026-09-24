import { describe, expect, it } from "vitest";

import { changelogText } from "./changelog";

describe("changelogText", () => {
  it("strips headings, emphasis and links", () => {
    expect(changelogText("## Changes\n- **Fixed** a [crash](https://x.y)\n* Updated `JEI`")).toBe(
      "Changes\n• Fixed a crash\n• Updated JEI",
    );
  });

  it("drops images and collapses blank lines", () => {
    expect(changelogText("Intro\n\n\n\n![banner](a.png)\n---\nEnd")).toBe("Intro\n\nEnd");
  });

  it("keeps plain text untouched", () => {
    expect(changelogText("Just some notes.")).toBe("Just some notes.");
  });
});
