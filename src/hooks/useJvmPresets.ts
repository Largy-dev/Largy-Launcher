import { useMutation, useQueryClient } from "@tanstack/react-query";

import { useSettings } from "@/hooks/useSettings";
import { settingsApi, type JvmPreset } from "@/services/tauri";

/**
 * Named memory/JVM combinations saved from one instance's settings and
 * reusable on any other — stored on the global settings object, so no
 * dedicated backend command is needed to add or remove one.
 */
export function useJvmPresets() {
  const { data: settings } = useSettings();
  const queryClient = useQueryClient();
  const presets = settings?.jvm_presets ?? [];

  const save = useMutation({
    mutationFn: (next: JvmPreset[]) => settingsApi.update({ ...settings!, jvm_presets: next }),
    onSuccess: (saved) => queryClient.setQueryData(["settings"], saved),
  });

  return {
    presets,
    addPreset: (preset: JvmPreset) => save.mutate([...presets.filter((p) => p.name !== preset.name), preset]),
    removePreset: (name: string) => save.mutate(presets.filter((p) => p.name !== name)),
    saving: save.isPending,
  };
}
