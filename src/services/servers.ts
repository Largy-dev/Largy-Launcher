import { invoke } from "@tauri-apps/api/core";

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
