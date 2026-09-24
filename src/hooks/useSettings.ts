import { useQuery } from "@tanstack/react-query";

import { providersApi, settingsApi } from "@/services/tauri";

export function useSettings() {
  return useQuery({ queryKey: ["settings"], queryFn: settingsApi.get });
}

/** Whether the build ships its own CurseForge API key (fixed for the app's lifetime). */
export function useCurseforgeBuiltinKey() {
  return useQuery({
    queryKey: ["curseforge-builtin-key"],
    queryFn: providersApi.curseforgeBuiltinKey,
    staleTime: Infinity,
  }).data;
}

/** CurseForge works with the build's own key or one the player entered. */
export function useCurseforgeEnabled() {
  const { data: settings } = useSettings();
  const builtin = useCurseforgeBuiltinKey();
  return !!builtin || !!settings?.curseforge_api_key.trim();
}
