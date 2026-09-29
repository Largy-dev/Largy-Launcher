import { useMutation, useQueryClient } from "@tanstack/react-query";
import { convertFileSrc } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { ImageIcon, ImageOff, Sparkles } from "lucide-react";

import type { CoverChoice } from "@/bindings/CoverChoice";
import { SettingRow } from "@/components/settings/SettingsKit";
import { Button } from "@/components/ui/button";
import { notify } from "@/lib/notify";
import { cn } from "@/lib/utils";
import { errorMessage, instancesApi, type Instance } from "@/services/tauri";

/** Mutation shared by the cover row and the screenshot viewer. */
export function useSetCover(instanceId: string) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (cover: CoverChoice) => instancesApi.setCover(instanceId, cover),
    onSuccess: (updated) => {
      queryClient.setQueryData(["instance", instanceId], updated);
      queryClient.invalidateQueries({ queryKey: ["instances"] });
    },
    onError: (e) => notify.error({ title: "Couverture non modifiée", message: errorMessage(e) }),
  });
}

/** The picture of the instance's card and banner. */
export function CoverRow({ instance }: { instance: Instance }) {
  const setCover = useSetCover(instance.id);
  const mode = instance.cover === null ? "auto" : instance.cover === "none" ? "none" : "file";

  async function pick() {
    const file = await open({
      multiple: false,
      filters: [{ name: "Image", extensions: ["png", "jpg", "jpeg", "webp"] }],
    });
    if (typeof file === "string") setCover.mutate({ file });
  }

  return (
    <SettingRow
      label="Couverture"
      description={
        mode === "auto"
          ? "Ta dernière capture d'écran (F2) s'affiche sur la carte de l'instance."
          : mode === "none"
            ? "Pas d'image : la carte garde les couleurs de son mod loader."
            : "Une image choisie par toi."
      }
      control={
        <div className="flex items-center gap-2">
          <div className="h-12 w-20 overflow-hidden rounded-lg bg-muted ring-1 ring-border/60">
            {instance.cover_path ? (
              <img src={convertFileSrc(instance.cover_path)} alt="" className="size-full object-cover" />
            ) : (
              <div className="flex size-full items-center justify-center text-muted-foreground">
                <ImageOff className="size-4" aria-hidden="true" />
              </div>
            )}
          </div>
          <div className="flex flex-col gap-1">
            <Button size="sm" variant="outline" className="h-7 gap-1.5" onClick={pick} disabled={setCover.isPending}>
              <ImageIcon aria-hidden="true" />
              Choisir…
            </Button>
            <div className="flex gap-1">
              <Button
                size="sm"
                variant="ghost"
                className={cn("h-6 gap-1 px-2 text-xs", mode === "auto" && "text-primary")}
                aria-pressed={mode === "auto"}
                onClick={() => setCover.mutate("auto")}
              >
                <Sparkles className="size-3" aria-hidden="true" />
                Auto
              </Button>
              <Button
                size="sm"
                variant="ghost"
                className={cn("h-6 px-2 text-xs", mode === "none" && "text-primary")}
                aria-pressed={mode === "none"}
                onClick={() => setCover.mutate("none")}
              >
                Aucune
              </Button>
            </div>
          </div>
        </div>
      }
    />
  );
}
