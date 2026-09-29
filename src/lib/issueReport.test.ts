import { describe, expect, it } from "vitest";

import { buildIssueUrl } from "./issueReport";

function bodyOf(url: string) {
  const params = new URL(url).searchParams;
  return { title: params.get("title"), body: params.get("body") ?? "", labels: params.get("labels") };
}

describe("buildIssueUrl", () => {
  it("puts the description first and the environment after", () => {
    const { title, body, labels } = bodyOf(
      buildIssueUrl({
        appVersion: "1.0.8",
        os: "Windows 11 Pro",
        description: "Le jeu plante au chargement\ndes détails",
        instance: { name: "ATM", minecraftVersion: "1.21.1", loader: "neoforge", loaderVersion: "21.1.77", mods: 320 },
        logUrls: ["https://mclo.gs/abc"],
      }),
    );
    expect(title).toBe("Le jeu plante au chargement");
    expect(labels).toBe("bug");
    expect(body.indexOf("Le jeu plante")).toBeLessThan(body.indexOf("Largy Launcher 1.0.8"));
    expect(body).toContain("Minecraft 1.21.1, neoforge 21.1.77, 320 mods");
    expect(body).toContain("- https://mclo.gs/abc");
  });

  it("falls back to the crash summary for the title and trims long text", () => {
    const url = buildIssueUrl({
      appVersion: "1",
      os: "Windows",
      description: "",
      crashSummary: "Mémoire insuffisante",
    });
    expect(bodyOf(url).title).toBe("Crash : Mémoire insuffisante");
    const long = buildIssueUrl({ appVersion: "1", os: "W", description: "x".repeat(10_000) });
    expect(long.length).toBeLessThan(8_000);
  });
});
