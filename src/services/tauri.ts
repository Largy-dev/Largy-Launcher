import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { AccountSession } from "@/bindings/AccountSession";
import type { CloseBehavior } from "@/bindings/CloseBehavior";
import type { ContentHit } from "@/bindings/ContentHit";
import type { ContentKind } from "@/bindings/ContentKind";
import type { CoverChoice } from "@/bindings/CoverChoice";
import type { CrashAnalysis } from "@/bindings/CrashAnalysis";
import type { DeviceCodeInfo } from "@/bindings/DeviceCodeInfo";
import type { DownloadProgress } from "@/bindings/DownloadProgress";
import type { ExportSummary } from "@/bindings/ExportSummary";
import type { GlobalSettings } from "@/bindings/GlobalSettings";
import type { InstallWarning } from "@/bindings/InstallWarning";
import type { Instance } from "@/bindings/Instance";
import type { InstanceExit } from "@/bindings/InstanceExit";
import type { InstanceInstallResult } from "@/bindings/InstanceInstallResult";
import type { InstanceLogBatch } from "@/bindings/InstanceLogBatch";
import type { InstanceSettingsInput } from "@/bindings/InstanceSettingsInput";
import type { JavaInstallation } from "@/bindings/JavaInstallation";
import type { JvmPreset } from "@/bindings/JvmPreset";
import type { LaunchPhase } from "@/bindings/LaunchPhase";
import type { LaunchPhaseEvent } from "@/bindings/LaunchPhaseEvent";
import type { LauncherBehavior } from "@/bindings/LauncherBehavior";
import type { LoaderKind } from "@/bindings/LoaderKind";
import type { LogLine } from "@/bindings/LogLine";
import type { MinecraftProfile } from "@/bindings/MinecraftProfile";
import type { ModUpdate } from "@/bindings/ModUpdate";
import type { ModpackDetails } from "@/bindings/ModpackDetails";
import type { ModpackRef } from "@/bindings/ModpackRef";
import type { ModpackSummary } from "@/bindings/ModpackSummary";
import type { ModpackVersionSummary } from "@/bindings/ModpackVersionSummary";
import type { PackCategory } from "@/bindings/PackCategory";
import type { PlaySession } from "@/bindings/PlaySession";
import type { ProcessStats } from "@/bindings/ProcessStats";
import type { SearchQuery } from "@/bindings/SearchQuery";
import type { SearchSort } from "@/bindings/SearchSort";
import type { Screenshot } from "@/bindings/Screenshot";
import type { StoredAccount } from "@/bindings/StoredAccount";
import type { SystemMemoryInfo } from "@/bindings/SystemMemoryInfo";
import type { VersionManifestEntry } from "@/bindings/VersionManifestEntry";

export type {
  AccountSession,
  CloseBehavior,
  ContentHit,
  ContentKind,
  CrashAnalysis,
  DeviceCodeInfo,
  DownloadProgress,
  ExportSummary,
  GlobalSettings,
  InstallWarning,
  Instance,
  InstanceExit,
  InstanceInstallResult,
  InstanceLogBatch,
  InstanceSettingsInput,
  JavaInstallation,
  JvmPreset,
  LaunchPhase,
  LaunchPhaseEvent,
  LauncherBehavior,
  LoaderKind,
  LogLine,
  MinecraftProfile,
  ModUpdate,
  ModpackDetails,
  ModpackRef,
  ModpackSummary,
  ModpackVersionSummary,
  PackCategory,
  SearchQuery,
  SearchSort,
  PlaySession,
  ProcessStats,
  Screenshot,
  StoredAccount,
  SystemMemoryInfo,
  VersionManifestEntry,
};

export interface AppError {
  kind: string;
  message: string;
}

export function isAppError(value: unknown): value is AppError {
  return typeof value === "object" && value !== null && "kind" in value && "message" in value;
}

/** The user cancelled the operation themselves — not worth an error toast. */
export function isCancelled(error: unknown): boolean {
  return isAppError(error) && error.kind === "cancelled";
}

export function errorMessage(error: unknown): string {
  if (isAppError(error)) return error.message;
  if (error instanceof Error) return error.message;
  return String(error);
}

// ---------------------------------------------------------------------------
// Shared domain types (mirrors the Rust structs — field names match exactly,
// since none of them opt into camelCase renaming).
// ---------------------------------------------------------------------------

/** `url`: a pack shared by link (an `.mrpack` online), kept in sync with the file. */
export type ProviderId = "modrinth" | "ftb" | "curseforge" | "url";

// ---------------------------------------------------------------------------
// Events
// ---------------------------------------------------------------------------

export function onDownloadProgress(handler: (p: DownloadProgress) => void): Promise<UnlistenFn> {
  return listen<DownloadProgress>("download-progress", (e) => handler(e.payload));
}

export function onInstanceLog(handler: (batch: InstanceLogBatch) => void): Promise<UnlistenFn> {
  return listen<InstanceLogBatch>("instance-log", (e) => handler(e.payload));
}

export function onInstanceExit(handler: (exit: InstanceExit) => void): Promise<UnlistenFn> {
  return listen<InstanceExit>("instance-exit", (e) => handler(e.payload));
}

export function onLaunchPhase(handler: (e: LaunchPhaseEvent) => void): Promise<UnlistenFn> {
  return listen<LaunchPhaseEvent>("launch-phase", (e) => handler(e.payload));
}

/** The window's close button was pressed and the user hasn't chosen what it does yet. */
export function onCloseRequested(handler: () => void): Promise<UnlistenFn> {
  return listen("close-requested", () => handler());
}

export function onInstancesChanged(handler: () => void): Promise<UnlistenFn> {
  return listen("instances-changed", () => handler());
}

/** A desktop shortcut was opened while the launcher was already running. */
export function onLaunchRequest(handler: (instanceId: string) => void): Promise<UnlistenFn> {
  return listen<string>("launch-request", (e) => handler(e.payload));
}

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

export function getAppVersion(): Promise<string> {
  return invoke<string>("app_version");
}

export function getSystemMemoryMb(): Promise<number> {
  return invoke<number>("system_memory_mb");
}

export function getSystemMemoryInfo(): Promise<SystemMemoryInfo> {
  return invoke<SystemMemoryInfo>("system_memory_info");
}

export function loadersListVersions(loader: LoaderKind, minecraftVersion: string): Promise<string[]> {
  return invoke<string[]>("loaders_list_versions", { loader, minecraftVersion });
}

export const javaApi = {
  list: () => invoke<JavaInstallation[]>("java_list_installations"),
  probe: (path: string) => invoke<JavaInstallation>("java_probe", { path }),
};

/** Reduce to the notification area (`tray`) or exit (`quit`), optionally remembering the choice. */
export function closeApp(action: Exclude<CloseBehavior, "ask">, remember: boolean): Promise<void> {
  return invoke<void>("app_close_action", { action, remember });
}

export function openLauncherLogs(): Promise<void> {
  return invoke<void>("open_launcher_logs");
}

export const auth = {
  beginLogin: () => invoke<DeviceCodeInfo>("auth_begin_login"),
  completeLogin: (device: DeviceCodeInfo) => invoke<AccountSession>("auth_complete_login", { device }),
  cancelLogin: () => invoke<void>("auth_cancel_login"),
  trySilentLogin: () => invoke<AccountSession | null>("auth_try_silent_login"),
  listAccounts: () => invoke<StoredAccount[]>("auth_list_accounts"),
  switchAccount: (accountId: string) => invoke<AccountSession>("auth_switch_account", { accountId }),
  /** Forgets `accountId`, or the active account when omitted. */
  logout: (accountId?: string) => invoke<void>("auth_logout", { accountId: accountId ?? null }),
  getActiveAccount: () => invoke<AccountSession | null>("auth_get_active_account"),
};

export const settingsApi = {
  get: () => invoke<GlobalSettings>("settings_get"),
  update: (settings: GlobalSettings) => invoke<GlobalSettings>("settings_update", { settings }),
  /** Size in bytes of the downloaded Forge/NeoForge installer jars. */
  installerCacheSize: () => invoke<number>("settings_installer_cache_size"),
  /** Deletes them; resolves to the number of bytes freed. */
  clearInstallerCache: () => invoke<number>("settings_clear_installer_cache"),
};

export const minecraftApi = {
  listVersions: () => invoke<VersionManifestEntry[]>("minecraft_list_versions"),
};

export const providersApi = {
  search: (provider: ProviderId, query: Partial<SearchQuery> & { text: string }) =>
    invoke<ModpackSummary[]>("providers_search", {
      provider,
      query: { offset: 0, game_version: null, loader: null, category: null, sort: "relevance", ...query },
    }),
  /** Categories this provider's modpacks can be filtered by (none for FTB). */
  categories: (provider: ProviderId) => invoke<PackCategory[]>("providers_categories", { provider }),
  getModpack: (provider: ProviderId, packId: string) =>
    invoke<ModpackDetails>("providers_get_modpack", { provider, packId }),
  getVersions: (provider: ProviderId, packId: string) =>
    invoke<ModpackVersionSummary[]>("providers_get_versions", { provider, packId }),
  getChangelog: (provider: ProviderId, packId: string, versionId: string) =>
    invoke<string | null>("providers_get_changelog", { provider, packId, versionId }),
  /** Whether this build ships its own CurseForge API key. */
  curseforgeBuiltinKey: () => invoke<boolean>("providers_curseforge_builtin_key"),
  /** The FTB-published copy of a CurseForge pack, if any. */
  ftbEquivalent: (curseforgeId: number, name: string) =>
    invoke<ModpackSummary | null>("providers_ftb_equivalent", { curseforgeId, name }),
};

export const instancesApi = {
  list: () => invoke<Instance[]>("instances_list"),
  get: (id: string) => invoke<Instance>("instances_get", { id }),
  create: (name: string, minecraftVersion: string, loader: LoaderKind, loaderVersion: string | null) =>
    invoke<Instance>("instances_create", { name, minecraftVersion, loader, loaderVersion }),
  delete: (id: string) => invoke<void>("instances_delete", { id }),
  rename: (id: string, name: string) => invoke<Instance>("instances_rename", { id, name }),
  duplicate: (id: string, name: string) => invoke<Instance>("instances_duplicate", { id, name }),
  updateSettings: (id: string, settings: InstanceSettingsInput) =>
    invoke<Instance>("instances_update_settings", { id, settings }),
  setPinned: (id: string, pinned: boolean) => invoke<Instance>("instances_set_pinned", { id, pinned }),
  setProtected: (id: string, protected_: boolean) =>
    invoke<Instance>("instances_set_protected", { id, protected: protected_ }),
  setNotes: (id: string, notes: string) => invoke<Instance>("instances_set_notes", { id, notes }),
  /** Card / banner picture: "auto" (newest screenshot), "none", or { file: path }. */
  setCover: (id: string, cover: CoverChoice) => invoke<Instance>("instances_set_cover", { id, cover }),
  /** Copies `sourceId`'s memory/JVM/window/server settings onto `targetId`. */
  copySettings: (sourceId: string, targetId: string) =>
    invoke<Instance>("instances_copy_settings", { sourceId, targetId }),
  /** Opens the instance folder, or one of its sub-folders (`mods`, `saves`, `backups`…). */
  openFolder: (id: string, sub?: string) => invoke<void>("instances_open_folder", { id, sub: sub ?? null }),
  revealFile: (id: string, path: string) => invoke<void>("instances_reveal_file", { id, path }),
  installModpack: (
    provider: ProviderId,
    packId: string,
    versionId: string,
    packName: string,
    packIconUrl: string | null,
    instanceName: string,
  ) =>
    invoke<InstanceInstallResult>("instances_install_modpack", {
      provider,
      packId,
      versionId,
      packName,
      packIconUrl,
      instanceName,
    }),
  cancelInstall: (id: string) => invoke<void>("instances_cancel_install", { id }),
  updateModpack: (instanceId: string, versionId: string) =>
    invoke<InstanceInstallResult>("instances_update_modpack", { instanceId, versionId }),
  import: (path: string) => invoke<InstanceInstallResult>("instances_import", { path }),
  /** Moves hand-downloaded files from Downloads into the instance; resolves to the paths now in place. */
  collectManualDownloads: (instanceId: string, files: { path: string; sha1: string | null }[]) =>
    invoke<string[]>("instances_collect_manual_downloads", { instanceId, files }),
  export: (id: string, dest: string, includeSaves: boolean) =>
    invoke<ExportSummary>("instances_export", { id, dest, includeSaves }),
  backupWorlds: (id: string) => invoke<string | null>("instances_backup_worlds", { id }),
  /** Puts a « play this instance » shortcut on the desktop; resolves to its path. */
  createShortcut: (id: string) => invoke<string>("instances_create_shortcut", { id }),
  screenshots: (id: string) => invoke<Screenshot[]>("instance_screenshots_list", { id }),
  deleteScreenshot: (id: string, fileName: string) => invoke<void>("instance_screenshots_delete", { id, fileName }),
};

export const launchApi = {
  /** `server` (`host[:port]`) joins that server for this launch only. */
  launch: (instanceId: string, server?: string) =>
    invoke<void>("launch_instance", { instanceId, server: server ?? null }),
  /** Instance a desktop shortcut asked to launch at startup, handed over once. */
  takePendingLaunch: () => invoke<string | null>("take_pending_launch"),
  /** Stops the game — or cancels a launch still preparing. */
  stop: (instanceId: string) => invoke<void>("stop_instance", { instanceId }),
  repair: (instanceId: string) => invoke<void>("repair_instance", { instanceId }),
  isRunning: (instanceId: string) => invoke<boolean>("is_instance_running", { instanceId }),
  stats: (instanceId: string) => invoke<ProcessStats | null>("instance_process_stats", { instanceId }),
};
