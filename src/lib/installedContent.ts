import type { InstalledItem } from "@/services/content";

import { satisfies } from "./versionRange";

/** "create-1.20.1-0.5.1f.jar" → "create 1.20.1 0.5.1f" — for files without metadata. */
export function prettyFileName(fileName: string): string {
  return fileName
    .replace(/\.(jar|zip)(\.disabled)?$/i, "")
    .replace(/[-_+]/g, " ")
    .trim();
}

/** What to call an installed file: its catalogue title, its own name, or its file name. */
export function displayName(item: InstalledItem): string {
  return item.remote?.title || item.name || prettyFileName(item.file_name);
}

export interface DependencyReport {
  /** Enabled mods → the mod ids they require that no enabled mod provides. */
  missing: Map<string, string[]>;
  /** Mod → names of the enabled mods that can't run without it. */
  requiredBy: Map<string, string[]>;
  /** Enabled mods → names of the enabled mods one of the two declares itself incompatible with. */
  conflicts: Map<string, string[]>;
}

/** Files with a missing dependency or a conflict. */
export function problemFiles(report: DependencyReport): Set<string> {
  return new Set([...report.missing.keys(), ...report.conflicts.keys()]);
}

/**
 * Cross-checks the enabled mods' declared dependencies. Disabled mods
 * neither provide nor require anything — the game doesn't load them.
 */
/**
 * Mods that rewrite another mod's metadata while the game loads, so what
 * its jar declares no longer holds. Monocle (shipped in FTB packs) lets Iris
 * run on Embeddium: Iris's Sodium requirement and Embeddium incompatibility
 * are lifted.
 */
const METADATA_PATCHES: { patcher: string; mod: string; depends: string[]; breaks: string[] }[] = [
  { patcher: "monocle", mod: "iris", depends: ["sodium"], breaks: ["embeddium"] },
];

function isPresent(items: InstalledItem[], id: string): boolean {
  return items.some(
    (i) => i.provides.includes(id) || i.mod_id === id || i.file_name.toLowerCase().startsWith(`${id}-`),
  );
}

/** The enabled mods, with the metadata patches of the installed patchers applied. */
function effectiveMods(items: InstalledItem[]): InstalledItem[] {
  const enabled = items.filter((i) => i.enabled);
  const patches = METADATA_PATCHES.filter((p) => isPresent(enabled, p.patcher));
  if (patches.length === 0) return enabled;
  return enabled.map((item) => {
    const applying = patches.filter((p) => item.mod_id === p.mod || item.provides.includes(p.mod));
    if (applying.length === 0) return item;
    return {
      ...item,
      depends: item.depends.filter((d) => !applying.some((p) => p.depends.includes(d))),
      breaks: item.breaks.filter((b) => !applying.some((p) => p.breaks.includes(b.id))),
    };
  });
}

export function analyseDependencies(items: InstalledItem[]): DependencyReport {
  const enabled = effectiveMods(items);
  const providers = new Map<string, InstalledItem[]>();
  for (const item of enabled) {
    for (const id of item.provides) providers.set(id, [...(providers.get(id) ?? []), item]);
  }

  const missing = new Map<string, string[]>();
  const requiredBy = new Map<string, string[]>();
  const conflicts = new Map<string, string[]>();
  const addConflict = (file: string, other: string) => {
    const list = conflicts.get(file) ?? [];
    if (!list.includes(other)) list.push(other);
    conflicts.set(file, list);
  };
  for (const item of enabled) {
    const absent = item.depends.filter((id) => !providers.has(id));
    if (absent.length > 0) missing.set(item.file_name, absent);
    for (const id of item.depends) {
      for (const provider of providers.get(id) ?? []) {
        if (provider.file_name === item.file_name) continue;
        const list = requiredBy.get(provider.file_name) ?? [];
        if (!list.includes(displayName(item))) list.push(displayName(item));
        requiredBy.set(provider.file_name, list);
      }
    }
    for (const rule of item.breaks) {
      for (const other of providers.get(rule.id) ?? []) {
        if (other.file_name === item.file_name) continue;
        // The declared range is about rule.id's version: known only when it is the file's own mod.
        const version = other.mod_id === rule.id ? other.version : null;
        if (!satisfies(version, rule.versions, rule.maven)) continue;
        addConflict(item.file_name, displayName(other));
        addConflict(other.file_name, displayName(item));
      }
    }
  }
  return { missing, requiredBy, conflicts };
}

/** Case- and accent-insensitive match on everything a player might type. */
export function matchesSearch(item: InstalledItem, needle: string): boolean {
  if (!needle) return true;
  const fold = (s: string) =>
    s
      .normalize("NFD")
      .replace(/\p{Diacritic}/gu, "")
      .toLowerCase();
  const hay = [item.file_name, item.name, item.remote?.title, item.mod_id, item.description, ...item.authors]
    .filter(Boolean)
    .join(" ");
  return fold(hay).includes(fold(needle.trim()));
}
