import { describe, expect, it } from "vitest";

import type { InstalledItem } from "@/services/content";

import { analyseDependencies, displayName, matchesSearch, prettyFileName } from "./installedContent";

function item(file_name: string, over: Partial<InstalledItem> = {}): InstalledItem {
  return {
    file_name,
    enabled: true,
    is_dir: false,
    size: 1,
    modified: 0,
    sha1: null,
    fingerprint: null,
    mod_id: null,
    name: null,
    version: null,
    description: null,
    authors: [],
    loaders: [],
    provides: [],
    depends: [],
    breaks: [],
    icon_path: null,
    remote: null,
    ...over,
  };
}

describe("displayName", () => {
  it("prefers the catalogue title, then the mod's own name, then the file name", () => {
    const remote = {
      provider: "modrinth" as const,
      project_id: "p",
      title: "Sodium",
      description: "",
      icon_url: null,
      url: "",
    };
    expect(displayName(item("a.jar", { name: "sodium", remote }))).toBe("Sodium");
    expect(displayName(item("a.jar", { name: "Iris" }))).toBe("Iris");
    expect(displayName(item("create-1.20.1-0.5.jar"))).toBe("create 1.20.1 0.5");
    expect(prettyFileName("pack_v2.zip.disabled")).toBe("pack v2");
  });
});

describe("analyseDependencies", () => {
  it("reports missing dependencies and who requires what", () => {
    const report = analyseDependencies([
      item("create.jar", { name: "Create", provides: ["create"], depends: ["flywheel"] }),
      item("flywheel.jar", { name: "Flywheel", provides: ["flywheel"] }),
      item("addon.jar", { name: "Addon", provides: ["addon"], depends: ["create", "missing-lib"] }),
    ]);
    expect(report.missing.get("addon.jar")).toEqual(["missing-lib"]);
    expect(report.missing.has("create.jar")).toBe(false);
    expect(report.requiredBy.get("flywheel.jar")).toEqual(["Create"]);
    expect(report.requiredBy.get("create.jar")).toEqual(["Addon"]);
  });

  it("reports declared incompatibilities on both sides", () => {
    const report = analyseDependencies([
      item("iris.jar", {
        name: "Iris",
        provides: ["iris"],
        breaks: [{ id: "embeddium", versions: null, maven: false }],
      }),
      item("embeddium.jar", { name: "Embeddium", provides: ["embeddium"] }),
      item("off.jar", {
        name: "Off",
        provides: ["off"],
        breaks: [{ id: "iris", versions: null, maven: false }],
        enabled: false,
      }),
      item("mek.jar", {
        name: "Mekanism",
        provides: ["mekanism"],
        breaks: [{ id: "appmek", versions: "(,1.6.2)", maven: true }],
      }),
      item("appmek.jar", { name: "AppMek", mod_id: "appmek", version: "1.6.3", provides: ["appmek"] }),
    ]);
    expect(report.conflicts.has("mek.jar")).toBe(false);
    expect(report.conflicts.get("iris.jar")).toEqual(["Embeddium"]);
    expect(report.conflicts.get("embeddium.jar")).toEqual(["Iris"]);
    expect(report.conflicts.has("off.jar")).toBe(false);
  });

  it("knows Monocle lets Iris run on Embeddium (FTB packs)", () => {
    const iris = item("iris.jar", {
      name: "Iris",
      mod_id: "iris",
      provides: ["iris"],
      depends: ["sodium"],
      breaks: [{ id: "embeddium", versions: null, maven: false }],
    });
    const embeddium = item("embeddium.jar", { name: "Embeddium", mod_id: "embeddium", provides: ["embeddium"] });
    const monocle = item("monocle-0.2.3.ms.jar", { provides: ["monocle"] });

    const without = analyseDependencies([iris, embeddium]);
    expect(without.missing.get("iris.jar")).toEqual(["sodium"]);
    expect(without.conflicts.get("iris.jar")).toEqual(["Embeddium"]);

    const withMonocle = analyseDependencies([iris, embeddium, monocle]);
    expect(withMonocle.missing.size).toBe(0);
    expect(withMonocle.conflicts.size).toBe(0);
  });

  it("ignores disabled mods on both sides", () => {
    const report = analyseDependencies([
      item("create.jar", { provides: ["create"], depends: ["flywheel"] }),
      item("flywheel.jar", { provides: ["flywheel"], enabled: false }),
      item("off.jar", { depends: ["nothing"], enabled: false }),
    ]);
    expect(report.missing.get("create.jar")).toEqual(["flywheel"]);
    expect(report.missing.has("off.jar")).toBe(false);
  });
});

describe("matchesSearch", () => {
  it("looks at names, ids, authors and ignores accents", () => {
    const mod = item("jei-1.20.jar", { name: "Just Enough Items", mod_id: "jei", authors: ["mezz"] });
    expect(matchesSearch(mod, "enough")).toBe(true);
    expect(matchesSearch(mod, "MEZZ")).toBe(true);
    expect(matchesSearch(item("a.jar", { description: "Améliore les performances" }), "ameliore")).toBe(true);
    expect(matchesSearch(mod, "sodium")).toBe(false);
  });
});
