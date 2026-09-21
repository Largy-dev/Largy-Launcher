import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export interface AppError {
  kind: string;
  message: string;
}

export function isAppError(value: unknown): value is AppError {
  return typeof value === "object" && value !== null && "kind" in value && "message" in value;
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
}

export interface GlobalSettings {
  default_min_memory_mb: number;
  default_max_memory_mb: number;
  default_jvm_args: string[];
  azure_client_id: string;
  curseforge_api_key: string;
  java_path_override: string | null;
}

export interface MinecraftProfile {
  id: string;
  name: string;
}

export interface AccountSession {
  profile: MinecraftProfile;
  minecraft_access_token: string;
  expires_at: number;
}

export interface DeviceCodeInfo {
  device_code: string;
  user_code: string;
  verification_uri: string;
  expires_in: number;
  interval: number;
}

export interface ModpackSummary {
  id: string;
  provider: string;
  name: string;
  author: string;
  icon_url: string | null;
  summary: string;
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

export type ProviderId = "ftb" | "curseforge";

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

export interface InstanceLogLine {
  instance_id: string;
  line: string;
  stream: "stdout" | "stderr";
}

export interface InstanceExit {
  instance_id: string;
  code: number | null;
}

export function onDownloadProgress(handler: (p: DownloadProgress) => void): Promise<UnlistenFn> {
  return listen<DownloadProgress>("download-progress", (e) => handler(e.payload));
}

export function onInstanceLog(handler: (line: InstanceLogLine) => void): Promise<UnlistenFn> {
  return listen<InstanceLogLine>("instance-log", (e) => handler(e.payload));
}

export function onInstanceExit(handler: (exit: InstanceExit) => void): Promise<UnlistenFn> {
  return listen<InstanceExit>("instance-exit", (e) => handler(e.payload));
}

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

export function getAppVersion(): Promise<string> {
  return invoke<string>("app_version");
}

export function loadersListVersions(loader: LoaderKind, minecraftVersion: string): Promise<string[]> {
  return invoke<string[]>("loaders_list_versions", { loader, minecraftVersion });
}

export const auth = {
  beginLogin: () => invoke<DeviceCodeInfo>("auth_begin_login"),
  completeLogin: (device: DeviceCodeInfo) => invoke<AccountSession>("auth_complete_login", { device }),
  trySilentLogin: () => invoke<AccountSession | null>("auth_try_silent_login"),
  logout: () => invoke<void>("auth_logout"),
  getActiveAccount: () => invoke<AccountSession | null>("auth_get_active_account"),
};

export const settingsApi = {
  get: () => invoke<GlobalSettings>("settings_get"),
  update: (settings: GlobalSettings) => invoke<void>("settings_update", { settings }),
};

export const minecraftApi = {
  listVersions: () => invoke<VersionManifestEntry[]>("minecraft_list_versions"),
};

export const providersApi = {
  search: (provider: ProviderId, text: string) => invoke<ModpackSummary[]>("providers_search", { provider, text }),
  getModpack: (provider: ProviderId, packId: string) =>
    invoke<ModpackDetails>("providers_get_modpack", { provider, packId }),
  getVersions: (provider: ProviderId, packId: string) =>
    invoke<ModpackVersionSummary[]>("providers_get_versions", { provider, packId }),
};

export const instancesApi = {
  list: () => invoke<Instance[]>("instances_list"),
  get: (id: string) => invoke<Instance>("instances_get", { id }),
  create: (name: string, minecraftVersion: string, loader: LoaderKind, loaderVersion: string | null) =>
    invoke<Instance>("instances_create", { name, minecraftVersion, loader, loaderVersion }),
  delete: (id: string) => invoke<void>("instances_delete", { id }),
  updateSettings: (
    id: string,
    minMemoryMb: number | null,
    maxMemoryMb: number | null,
    extraJvmArgs: string[],
  ) => invoke<Instance>("instances_update_settings", { id, minMemoryMb, maxMemoryMb, extraJvmArgs }),
  openFolder: (id: string) => invoke<void>("instances_open_folder", { id }),
  installModpack: (
    provider: ProviderId,
    packId: string,
    versionId: string,
    packName: string,
    instanceName: string,
  ) =>
    invoke<Instance>("instances_install_modpack", {
      provider,
      packId,
      versionId,
      packName,
      instanceName,
    }),
};

export const launchApi = {
  launch: (instanceId: string) => invoke<void>("launch_instance", { instanceId }),
  stop: (instanceId: string) => invoke<void>("stop_instance", { instanceId }),
  isRunning: (instanceId: string) => invoke<boolean>("is_instance_running", { instanceId }),
};
