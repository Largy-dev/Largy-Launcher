import { invoke } from "@tauri-apps/api/core";

import type { Instance } from "./tauri";

/** One entry of an instance's `servers.dat`, in the in-game order. */
export interface ServerEntry {
  name: string;
  address: string;
  /** Base64 PNG the game cached from the server's last ping. */
  icon: string | null;
}

export interface ServerStatus {
  online: number;
  max: number;
  version: string;
  /** `§`-coded description (see `lib/motd`). */
  motd: string;
  favicon: string | null;
  latency_ms: number;
  players: string[];
}

export const serversApi = {
  list: (instanceId: string) => invoke<ServerEntry[]>("instance_servers_list", { instanceId }),
  add: (instanceId: string, name: string, address: string) =>
    invoke<void>("instance_servers_add", { instanceId, name, address }),
  update: (instanceId: string, index: number, name: string, address: string) =>
    invoke<void>("instance_servers_update", { instanceId, index, name, address }),
  remove: (instanceId: string, index: number) => invoke<void>("instance_servers_remove", { instanceId, index }),
  ping: (address: string) => invoke<ServerStatus>("server_ping", { address }),
};

/** A pack of client mods offered when preparing a server instance. */
export interface ServerPreset {
  id: string;
  label: string;
  description: string;
  default: boolean;
  /** Modrinth slugs (Fabric mods). */
  mods: string[];
  /** Modrinth shader pack slugs; the first is switched on. */
  shaders: string[];
}

export interface FeaturedServer {
  id: string;
  name: string;
  address: string;
  description: string;
  tags: string[];
  language: string;
  minecraft_version: string;
  website: string | null;
  required_mods: string[];
  /** Modded servers: the exact modpack version to install instead of the Fabric presets. */
  modpack: ServerModpack | null;
}

export interface ServerModpack {
  provider: "ftb" | "modrinth" | "curseforge";
  pack_id: string;
  version_id: string;
  name: string;
  version_name: string;
}

export interface ServerCatalog {
  schema: number;
  presets: ServerPreset[];
  servers: FeaturedServer[];
}

export interface PrepareSpec {
  /** Catalog id, or `custom` for a server the player typed in. */
  featured_id: string;
  name: string;
  address: string;
  minecraft_version: string;
  mods: string[];
  shaders: string[];
  icon: string | null;
}

export interface PrepareResult {
  instance: Instance;
  warnings: string[];
}

export const catalogApi = {
  load: () => invoke<ServerCatalog>("featured_servers"),
  /** Creates a Fabric instance for the server: mods, shaders, server list, auto-join. */
  prepare: (spec: PrepareSpec) => invoke<PrepareResult>("servers_prepare_instance", { spec }),
  /** Links an installed modpack instance to its server: server list + auto-join. */
  attach: (instanceId: string, featuredId: string, address: string) =>
    invoke<Instance>("servers_attach_instance", { instanceId, featuredId, address }),
};
