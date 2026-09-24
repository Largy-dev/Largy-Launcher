import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

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

export type LoaderKind = "vanilla" | "forge" | "neoforge" | "fabric" | "quilt";

export interface VersionManifestEntry {
  id: string;
  type: string;
  url: string;
}

export interface ModpackRef {
  provider: string;
  pack_id: string;
  version_id: string;
  pack_name: string;
  installed_files: string[];
}

export interface Instance {
  id: string;
  name: string;
  minecraft_version: string;
  loader: LoaderKind;
  loader_version: string | null;
  directory: string;
  icon_url: string | null;
  min_memory_mb: number | null;
  max_memory_mb: number | null;
  extra_jvm_args: string[];
  modpack: ModpackRef | null;
  created_at: number;
  last_played_at: number | null;
  play_time_seconds: number;
  java_path: string | null;
  window_width: number | null;
  window_height: number | null;
  fullscreen: boolean;
  auto_join_server: string | null;
}

export interface InstanceSettingsInput {
  min_memory_mb: number | null;
  max_memory_mb: number | null;
  extra_jvm_args: string[];
  java_path: string | null;
  window_width: number | null;
  window_height: number | null;
  fullscreen: boolean;
  auto_join_server: string | null;
}

export type LauncherBehavior = "keep_open" | "minimize" | "hide";
export type CloseBehavior = "ask" | "tray" | "quit";

export interface GlobalSettings {
  default_min_memory_mb: number;
  default_max_memory_mb: number;
  default_jvm_args: string[];
  azure_client_id: string;
  curseforge_api_key: string;
  java_path_override: string | null;
  offline_mode: boolean;
  offline_username: string;
  on_game_launch: LauncherBehavior;
  on_close: CloseBehavior;
  discord_rich_presence: boolean;
}

export interface MinecraftProfile {
  id: string;
  name: string;
}

/** The active account as the backend exposes it (the game token stays in Rust). */
export interface AccountSession {
  profile: MinecraftProfile;
  /** Restored without network: singleplayer only until the next refresh. */
  offline: boolean;
}

export interface StoredAccount {
  id: string;
  name: string;
  active: boolean;
}

export interface DeviceCodeInfo {
  device_code: string;
  user_code: string;
  verification_uri: string;
  expires_in: number;
  interval: number;
}

export interface JavaInstallation {
  path: string;
  version: string;
  major: number;
  source: "managed" | "system";
}

export interface ModpackSummary {
  id: string;
  provider: string;
  name: string;
  author: string;
  icon_url: string | null;
  summary: string;
  downloads: number | null;
}

export interface ModpackDetails {
  summary: ModpackSummary;
  description: string;
}

export interface ModpackVersionSummary {
  id: string;
  name: string;
  minecraft_version: string;
  loader: LoaderKind;
  loader_version: string;
}

export type ProviderId = "modrinth" | "ftb" | "curseforge";

export interface InstallWarning {
  file_name: string;
  message: string;
  browser_url: string | null;
}

export interface InstanceInstallResult {
  instance: Instance;
  warnings: InstallWarning[];
}

export interface ExportSummary {
  path: string;
  referenced: number;
  bundled: number;
}

export type ContentKind = "mod" | "resource_pack" | "shader";

export interface ContentHit {
  project_id: string;
  slug: string;
  title: string;
  description: string;
  author: string;
  icon_url: string | null;
  downloads: number;
  project_type: string;
}

export interface ModUpdate {
  file_name: string;
  project_id: string;
  title: string;
  icon_url: string | null;
  current_version: string;
  new_version: string;
  new_file_name: string;
  url: string;
  sha1: string;
  size: number;
}

// ---------------------------------------------------------------------------
// Events
// ---------------------------------------------------------------------------

export interface DownloadProgress {
  task_id: string;
  label: string;
  bytes_done: number;
  bytes_total: number;
  files_done: number;
  files_total: number;
}

export interface LogLine {
  line: string;
  stream: "stdout" | "stderr";
}

export interface InstanceLogBatch {
  instance_id: string;
  lines: LogLine[];
}

export interface CrashAnalysis {
  summary: string;
  suggestion: string | null;
  matched_pattern: string;
  crash_report: string | null;
}

export type LaunchPhase = "auth" | "version" | "loader" | "natives" | "java" | "starting" | "running";

export interface LaunchPhaseEvent {
  instance_id: string;
  phase: LaunchPhase;
}

export interface ProcessStats {
  memory_mb: number;
  cpu_percent: number;
}

export interface SystemMemoryInfo {
  total_mb: number;
  available_mb: number;
}

export interface InstanceExit {
  instance_id: string;
  code: number | null;
  crash_analysis: CrashAnalysis | null;
  killed: boolean;
}

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
};

export const minecraftApi = {
  listVersions: () => invoke<VersionManifestEntry[]>("minecraft_list_versions"),
};

export const providersApi = {
  search: (provider: ProviderId, text: string, offset = 0) =>
    invoke<ModpackSummary[]>("providers_search", { provider, text, offset }),
  getModpack: (provider: ProviderId, packId: string) =>
    invoke<ModpackDetails>("providers_get_modpack", { provider, packId }),
  getVersions: (provider: ProviderId, packId: string) =>
    invoke<ModpackVersionSummary[]>("providers_get_versions", { provider, packId }),
  getChangelog: (provider: ProviderId, packId: string, versionId: string) =>
    invoke<string | null>("providers_get_changelog", { provider, packId, versionId }),
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
  export: (id: string, dest: string, includeSaves: boolean) =>
    invoke<ExportSummary>("instances_export", { id, dest, includeSaves }),
  backupWorlds: (id: string) => invoke<string | null>("instances_backup_worlds", { id }),
  /** Puts a « play this instance » shortcut on the desktop; resolves to its path. */
  createShortcut: (id: string) => invoke<string>("instances_create_shortcut", { id }),
  screenshots: (id: string) => invoke<Screenshot[]>("instance_screenshots_list", { id }),
  deleteScreenshot: (id: string, fileName: string) => invoke<void>("instance_screenshots_delete", { id, fileName }),
};

export interface Screenshot {
  file_name: string;
  path: string;
  size: number;
  taken_at: number;
}

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

export interface ModEntry {
  file_name: string;
  enabled: boolean;
  size: number;
}

export const instanceModsApi = {
  list: (instanceId: string) => invoke<ModEntry[]>("instance_mods_list", { instanceId }),
  setEnabled: (instanceId: string, fileName: string, enabled: boolean) =>
    invoke<void>("instance_mods_set_enabled", { instanceId, fileName, enabled }),
  delete: (instanceId: string, fileName: string) => invoke<void>("instance_mods_delete", { instanceId, fileName }),
  add: (instanceId: string, sourcePaths: string[]) => invoke<void>("instance_mods_add", { instanceId, sourcePaths }),
  checkUpdates: (instanceId: string) => invoke<ModUpdate[]>("instance_mods_check_updates", { instanceId }),
  /** Resolves to the file names that failed to update. */
  applyUpdates: (instanceId: string, updates: ModUpdate[]) =>
    invoke<string[]>("instance_mods_apply_updates", { instanceId, updates }),
};

export const contentApi = {
  search: (instanceId: string, kind: ContentKind, query: string, offset = 0) =>
    invoke<ContentHit[]>("content_search", { instanceId, kind, query, offset }),
  install: (instanceId: string, projectId: string, kind: ContentKind) =>
    invoke<string[]>("content_install", { instanceId, projectId, kind }),
};
