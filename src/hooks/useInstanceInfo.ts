import { useEffect, useMemo, useState } from "react";
import { useQuery } from "@tanstack/react-query";

import { useSettings } from "@/hooks/useSettings";
import { adviseRam, ramStatus } from "@/lib/ramAdvice";
import { getSystemMemoryInfo, instanceModsApi, type Instance } from "@/services/tauri";

export function useSystemMemory() {
  return useQuery({ queryKey: ["system-memory-info"], queryFn: getSystemMemoryInfo, staleTime: 60_000 });
}

/** Enabled mods in an instance (null for vanilla or while loading). Shares its cache with the mods screen. */
export function useModCount(instance: Instance | undefined): number | null {
  const modded = !!instance && instance.loader !== "vanilla";
  const { data } = useQuery({
    queryKey: ["instance-mods", instance?.id],
    queryFn: () => instanceModsApi.list(instance!.id),
    enabled: modded,
    staleTime: 60_000,
  });
  if (!modded || !data) return null;
  return data.filter((m) => m.enabled).length;
}

/** RAM actually allocated to an instance (its own setting, else the global default). */
export function useAllocatedRam(instance: Instance | undefined): number {
  const { data: settings } = useSettings();
  return instance?.max_memory_mb ?? settings?.default_max_memory_mb ?? 4096;
}

export function useRamAdvice(instance: Instance | undefined, modCount: number | null) {
  const { data: memory } = useSystemMemory();
  const allocated = useAllocatedRam(instance);
  return useMemo(() => {
    if (!instance) return null;
    const advice = adviseRam({
      loader: instance.loader,
      modCount,
      minecraftVersion: instance.minecraft_version,
      isModpack: !!instance.modpack,
      systemTotalMb: memory?.total_mb ?? null,
    });
    return { advice, allocated, ...ramStatus(allocated, advice) };
  }, [instance, modCount, memory?.total_mb, allocated]);
}

/** Re-renders every `intervalMs` — for live timers. */
export function useNow(intervalMs = 1000, enabled = true): number {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    if (!enabled) return;
    const id = window.setInterval(() => setNow(Date.now()), intervalMs);
    return () => window.clearInterval(id);
  }, [intervalMs, enabled]);
  return now;
}
