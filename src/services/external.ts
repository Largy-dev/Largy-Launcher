import { invoke } from "@tauri-apps/api/core";

import type { ExternalInstance } from "@/bindings/ExternalInstance";
import type { ExternalSource } from "@/bindings/ExternalSource";

import type { Instance } from "./tauri";

export type { ExternalInstance, ExternalSource };

/** Instances of other launchers (official, Prism / MultiMC, CurseForge app, Modrinth App). */
export const externalApi = {
  detect: () => invoke<ExternalInstance[]>("external_instances_detect"),
  /** Copies one of them (mods, configs, packs, options — worlds on request) into a new instance. */
  import: (id: string, includeWorlds: boolean) => invoke<Instance>("external_instance_import", { id, includeWorlds }),
};

export const EXTERNAL_SOURCE_LABEL: Record<ExternalSource, string> = {
  official: "Launcher officiel",
  prism: "Prism / MultiMC",
  curseforge: "CurseForge",
  modrinth: "Modrinth App",
};
