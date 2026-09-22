import { describe, expect, it } from "vitest";

import { adviseRam, minecraftMinor, ramStatus, type RamAdviceInput } from "./ramAdvice";

const input = (patch: Partial<RamAdviceInput>): RamAdviceInput => ({
  loader: "vanilla",
  modCount: null,
  minecraftVersion: "1.21.1",
  isModpack: false,
  systemTotalMb: 32768,
  ...patch,
});

describe("minecraftMinor", () => {
  it("reads the minor version and treats snapshots as recent", () => {
    expect(minecraftMinor("1.12.2")).toBe(12);
    expect(minecraftMinor("1.20.1")).toBe(20);
    expect(minecraftMinor("24w14a")).toBe(21);
  });
});

describe("adviseRam", () => {
  it("keeps vanilla modest, and lower still on old versions", () => {
    expect(adviseRam(input({})).recommendedMb).toBe(3072);
    expect(adviseRam(input({ minecraftVersion: "1.8.9" })).recommendedMb).toBe(2048);
  });

  it("scales Forge/NeoForge with the mod count, plus 1 Go from 1.18", () => {
    const small = adviseRam(input({ loader: "neoforge", modCount: 30, minecraftVersion: "1.16.5" }));
    const big = adviseRam(input({ loader: "neoforge", modCount: 300, minecraftVersion: "1.16.5" }));
    const modern = adviseRam(input({ loader: "forge", modCount: 200, minecraftVersion: "1.20.1" }));
    expect(small.recommendedMb).toBe(4608);
    expect(big.recommendedMb).toBe(10240);
    expect(modern.recommendedMb).toBe(8192 + 1024);
  });

  it("stays lighter for Fabric/Quilt", () => {
    expect(adviseRam(input({ loader: "fabric", modCount: 100 })).recommendedMb).toBe(5120);
  });

  it("assumes a sizeable pack when a modpack's mod count is unknown", () => {
    const advice = adviseRam(input({ loader: "neoforge", isModpack: true }));
    expect(advice.recommendedMb).toBe(6144 + 1024);
    expect(advice.reasons[0]).toContain("estimés");
  });

  it("caps the recommendation to what the PC can spare", () => {
    const advice = adviseRam(input({ loader: "forge", modCount: 300, systemTotalMb: 8192 }));
    expect(advice.maxSafeMb).toBe(4608);
    expect(advice.recommendedMb).toBeLessThanOrEqual(4608);
    expect(advice.reasons.some((r) => r.includes("Ton PC"))).toBe(true);
  });
});

describe("ramStatus", () => {
  const advice = adviseRam(
    input({ loader: "neoforge", modCount: 100, minecraftVersion: "1.20.1", systemTotalMb: 16384 }),
  );

  it("classifies too little, fine, too much and over the PC's limit", () => {
    expect(ramStatus(2048, advice).status).toBe("low");
    expect(ramStatus(advice.recommendedMb, advice).status).toBe("ok");
    expect(ramStatus(advice.maxSafeMb! + 512, advice).status).toBe("danger");
  });

  it("flags huge allocations even on big machines", () => {
    const roomy = adviseRam(input({ loader: "neoforge", modCount: 300, systemTotalMb: 65536 }));
    expect(ramStatus(20480, roomy).status).toBe("high");
  });
});
