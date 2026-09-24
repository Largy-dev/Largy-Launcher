import { describe, expect, it } from "vitest";

import { FRONT_HEIGHT, FRONT_WIDTH, frontParts } from "./skinPreview";

describe("frontParts", () => {
  it("keeps every part inside the front view", () => {
    for (const variant of ["classic", "slim"] as const) {
      for (const legacy of [false, true]) {
        for (const p of frontParts(variant, legacy)) {
          expect(p.dx).toBeGreaterThanOrEqual(0);
          expect(p.dx + p.w).toBeLessThanOrEqual(FRONT_WIDTH);
          expect(p.dy + p.h).toBeLessThanOrEqual(FRONT_HEIGHT);
          expect(p.sy + p.h).toBeLessThanOrEqual(legacy ? 32 : 64);
        }
      }
    }
  });

  it("slim arms are 3 pixels wide and hug the body", () => {
    const arms = frontParts("slim", false).filter((p) => p.w === 3);
    expect(arms.map((p) => p.dx).sort()).toEqual([1, 1, 12, 12]);
  });

  it("legacy skins mirror the right limbs and only overlay the hat", () => {
    const parts = frontParts("classic", true);
    expect(parts.filter((p) => p.flip)).toHaveLength(2);
    expect(parts).toHaveLength(7);
  });
});
