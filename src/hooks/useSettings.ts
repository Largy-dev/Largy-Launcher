import { useQuery } from "@tanstack/react-query";

import { settingsApi } from "@/services/tauri";

export function useSettings() {
  return useQuery({ queryKey: ["settings"], queryFn: settingsApi.get });
}
