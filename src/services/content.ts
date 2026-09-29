import { invoke } from "@tauri-apps/api/core";

import type { ContentHit } from "@/bindings/ContentHit";
import type { ContentKind } from "@/bindings/ContentKind";
import type { ContentSummary } from "@/bindings/ContentSummary";
import type { InstalledItem } from "@/bindings/InstalledItem";
import type { ModUpdate } from "@/bindings/ModUpdate";
import type { RemoteProject } from "@/bindings/RemoteProject";
import type { RemoteProvider } from "@/bindings/RemoteProvider";
import type { Snapshot } from "@/bindings/Snapshot";
import type { World } from "@/bindings/World";
import type { WorldBackup } from "@/bindings/WorldBackup";

import type { Instance } from "./tauri";

export type { ContentSummary, InstalledItem, RemoteProject, RemoteProvider, Snapshot, World, WorldBackup };

/** What's installed in an instance's mods / resource packs / shader packs folders. */
export const installedApi = {
  list: (instanceId: string, kind: ContentKind) =>
    invoke<InstalledItem[]>("instance_content_list", { instanceId, kind }),
  /** Counts only (mods, packs, shaders, worlds) — cheap, nothing is read or hashed. */
  summary: (instanceId: string) => invoke<ContentSummary>("instance_content_summary", { instanceId }),
  /** Looks up the Modrinth / CurseForge project of unidentified files; resolves to how many were found. */
  identify: (instanceId: string, kind: ContentKind) =>
    invoke<number>("instance_content_identify", { instanceId, kind }),
  /** Resolves to `"name : error"` for each file that couldn't be changed. */
  setEnabled: (instanceId: string, kind: ContentKind, fileNames: string[], enabled: boolean) =>
    invoke<string[]>("instance_content_set_enabled", { instanceId, kind, fileNames, enabled }),
  delete: (instanceId: string, kind: ContentKind, fileNames: string[]) =>
    invoke<string[]>("instance_content_delete", { instanceId, kind, fileNames }),
  /** Copies files from disk; resolves to the names added. */
  add: (instanceId: string, kind: ContentKind, sourcePaths: string[]) =>
    invoke<string[]>("instance_content_add", { instanceId, kind, sourcePaths }),
};

/** Modrinth / CurseForge catalogues for one instance, and mod updates. */
export const contentApi = {
  /** Whether CurseForge can be browsed (this build or the player has an API key). */
  curseforgeAvailable: () => invoke<boolean>("content_curseforge_available"),
  search: (instanceId: string, provider: RemoteProvider, kind: ContentKind, query: string, offset = 0) =>
    invoke<ContentHit[]>("content_search", { instanceId, provider, kind, query, offset }),
  /** Installs a project and its required dependencies; resolves to the files written. */
  install: (instanceId: string, provider: RemoteProvider, projectId: string, kind: ContentKind) =>
    invoke<string[]>("content_install", { instanceId, provider, projectId, kind }),
  checkUpdates: (instanceId: string) => invoke<ModUpdate[]>("instance_mods_check_updates", { instanceId }),
  /** Resolves to the file names that failed to update. */
  applyUpdates: (instanceId: string, updates: ModUpdate[]) =>
    invoke<string[]>("instance_mods_apply_updates", { instanceId, updates }),
};

export const worldsApi = {
  list: (instanceId: string) => invoke<World[]>("instance_worlds_list", { instanceId }),
  backup: (instanceId: string, folder: string) => invoke<WorldBackup>("instance_world_backup", { instanceId, folder }),
  /** Deletes a world, zipping it into its backups first when `backupFirst`. */
  delete: (instanceId: string, folder: string, backupFirst: boolean) =>
    invoke<void>("instance_world_delete", { instanceId, folder, backupFirst }),
  openFolder: (instanceId: string, folder: string) =>
    invoke<void>("instance_world_open_folder", { instanceId, folder }),
  /** Zips or world folders; resolves to the folders created in `saves/`. */
  import: (instanceId: string, sourcePaths: string[]) =>
    invoke<string[]>("instance_worlds_import", { instanceId, sourcePaths }),
  backups: (instanceId: string) => invoke<WorldBackup[]>("instance_world_backups", { instanceId }),
  /** Never overwrites a world: resolves to the folders created. */
  restore: (instanceId: string, backupId: string) =>
    invoke<string[]>("instance_world_restore", { instanceId, backupId }),
  deleteBackup: (instanceId: string, backupId: string) =>
    invoke<void>("instance_world_backup_delete", { instanceId, backupId }),
};

/** Restore points: taken before mod / modpack updates, or by hand. */
export const snapshotsApi = {
  list: (instanceId: string) => invoke<Snapshot[]>("instance_snapshots_list", { instanceId }),
  create: (instanceId: string) => invoke<Snapshot>("instance_snapshot_create", { instanceId }),
  /** The current state becomes a restore point first; resolves to the restored instance. */
  restore: (instanceId: string, snapshotId: string) =>
    invoke<Instance>("instance_snapshot_restore", { instanceId, snapshotId }),
  delete: (instanceId: string, snapshotId: string) =>
    invoke<void>("instance_snapshot_delete", { instanceId, snapshotId }),
};
