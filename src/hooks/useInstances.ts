import { useQuery } from "@tanstack/react-query";

import { instancesApi, providersApi, type Instance, type ProviderId } from "@/services/tauri";
import { useAppStore } from "@/store/appStore";

/** Every instance, kept fresh by the backend's `instances-changed` event (see useAppEvents). */
export function useInstances() {
  return useQuery({ queryKey: ["instances"], queryFn: instancesApi.list });
}

/** Most recently played (or created) first. */
export function byRecent(a: Instance, b: Instance): number {
  return (b.last_played_at ?? b.created_at) - (a.last_played_at ?? a.created_at);
}

/**
 * The instance the launcher is "about": the one picked in the sidebar, else
 * the last played. Drives the home screen, the sidebar highlight and the
 * backdrop.
 */
export function useSelectedInstance(): Instance | undefined {
  const activeId = useAppStore((s) => s.activeInstanceId);
  const { data: instances } = useInstances();
  if (!instances || instances.length === 0) return undefined;
  const active = activeId ? instances.find((i) => i.id === activeId) : undefined;
  return active ?? [...instances].sort(byRecent)[0];
}

/** The newest version of an installed modpack, when it isn't the one installed. */
export function useModpackUpdate(instance: Instance | undefined) {
  const modpack = instance?.modpack;
  const { data } = useQuery({
    queryKey: ["modpack-versions", modpack?.provider, modpack?.pack_id],
    queryFn: () => providersApi.getVersions(modpack!.provider as ProviderId, modpack!.pack_id),
    enabled: !!modpack,
    staleTime: 5 * 60 * 1000,
  });
  const latest = data?.[0];
  return !!modpack && !!latest && latest.id !== modpack.version_id ? latest : null;
}
