import { describe, expect, it } from "vitest";

import { motdPlainText, parseMotd, parseMotdLines } from "./motd";

describe("parseMotd", () => {
  it("keeps plain text as one unstyled run", () => {
    expect(parseMotd("A Minecraft Server")).toEqual([
      { text: "A Minecraft Server", color: null, bold: false, italic: false, underline: false, strike: false },
    ]);
  });

  it("applies colours, formats and resets", () => {
    const [a, b, c] = parseMotd("§6§lGold§r plain §#3AA9FFhex");
    expect(a).toMatchObject({ text: "Gold", color: "#ffaa00", bold: true });
    expect(b).toMatchObject({ text: " plain ", color: null, bold: false });
    expect(c).toMatchObject({ text: "hex", color: "#3aa9ff" });
  });

  it("a colour code clears formatting, like the game", () => {
    const [, second] = parseMotd("§lA§cB");
    expect(second).toMatchObject({ text: "B", color: "#ff5555", bold: false });
  });

  it("strips codes for the plain version", () => {
    expect(motdPlainText("§r§aHypixel §c[1.8]\n§fline")).toBe("Hypixel [1.8]\nline");
  });
});

describe("parseMotdLines", () => {
  it("splits lines and drops the centring spaces", () => {
    const lines = parseMotdLines("§r§f        §aHypixel §c[1.8]\n§f   §6§lSKYBLOCK   ");
    expect(lines.map((l) => l.map((s) => s.text).join(""))).toEqual(["Hypixel [1.8]", "SKYBLOCK"]);
    expect(lines[1][0]).toMatchObject({ color: "#ffaa00", bold: true });
  });

  it("skips empty lines", () => {
    expect(parseMotdLines("\n  \nHi")).toHaveLength(1);
  });
});
