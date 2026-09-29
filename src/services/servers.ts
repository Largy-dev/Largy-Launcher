import { invoke } from "@tauri-apps/api/core";

import type { InstanceInstallResult } from "./tauri";
import type { FeaturedServer } from "@/bindings/FeaturedServer";
import type { PrepareResult } from "@/bindings/PrepareResult";
import type { PrepareSpec } from "@/bindings/PrepareSpec";
import type { ServerCatalog } from "@/bindings/ServerCatalog";
import type { ServerEntry } from "@/bindings/ServerEntry";
import type { ServerModpack } from "@/bindings/ServerModpack";
import type { ServerPreset } from "@/bindings/ServerPreset";
import type { ServerStatus } from "@/bindings/ServerStatus";

export type {
  FeaturedServer,
  PrepareResult,
  PrepareSpec,
  ServerCatalog,
  ServerEntry,
  ServerModpack,
  ServerPreset,
  ServerStatus,
};

export const serversApi = {
  list: (instanceId: string) => invoke<ServerEntry[]>("instance_servers_list", { instanceId }),
  add: (instanceId: string, name: string, address: string) =>
    invoke<void>("instance_servers_add", { instanceId, name, address }),
  update: (instanceId: string, index: number, name: string, address: string) =>
    invoke<void>("instance_servers_update", { instanceId, index, name, address }),
  remove: (instanceId: string, index: number) => invoke<void>("instance_servers_remove", { instanceId, index }),
  ping: (address: string) => invoke<ServerStatus>("server_ping", { address }),
};

export const catalogApi = {
  load: () => invoke<ServerCatalog>("featured_servers"),
  /** Creates a Fabric instance for the server: mods, shaders, server list, auto-join. */
  prepare: (spec: PrepareSpec) => invoke<PrepareResult>("servers_prepare_instance", { spec }),
  /** Links an installed modpack instance to its server: server list, auto-join, the server's extra mods. */
  attach: (instanceId: string, featuredId: string, address: string) =>
    invoke<InstanceInstallResult>("servers_attach_instance", { instanceId, featuredId, address }),
};
