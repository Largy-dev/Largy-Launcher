import { useState } from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { useNavigate } from "react-router";
import { Blocks, FolderOpen, Loader2, Play, Settings2, Square, Trash2 } from "lucide-react";
import { toast } from "sonner";

import { Button } from "@/components/ui/button";
import { Card, CardDescription, CardFooter, CardHeader, CardTitle } from "@/components/ui/card";
import { Progress } from "@/components/ui/progress";
import { MinecraftGrassIcon } from "@/components/MinecraftGrassIcon";
import { useSettings } from "@/hooks/useSettings";
import { cn } from "@/lib/utils";
import { errorMessage, instancesApi, launchApi, type Instance } from "@/services/tauri";
import { useAppStore } from "@/store/appStore";

export function InstanceCard({ instance }: { instance: Instance }) {
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const account = useAppStore((s) => s.account);
  const running = useAppStore((s) => s.runtime[instance.id]?.running ?? false);
  const setRunning = useAppStore((s) => s.setRunning);
  const clearLogs = useAppStore((s) => s.clearLogs);
  const downloadProgress = useAppStore((s) => s.downloadProgress);
  const [busy, setBusy] = useState(false);
  const { data: settings } = useSettings();

  const installing =
    downloadProgress?.task_id === instance.id &&
    downloadProgress.files_total > 0 &&
    downloadProgress.files_done < downloadProgress.files_total;
  const installPercent =
    installing && downloadProgress && downloadProgress.bytes_total > 0
      ? Math.round((downloadProgress.bytes_done / downloadProgress.bytes_total) * 100)
      : 0;

  const deleteMutation = useMutation({
    mutationFn: () => instancesApi.delete(instance.id),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ["instances"] }),
    onError: (e) => toast.error(errorMessage(e)),
  });

  async function play() {
    if (!account && !settings?.offline_mode) {
      toast.error(
        "Connecte-toi avec ton compte Microsoft avant de jouer, ou active le Mode Hors-ligne dans Paramètres.",
      );
      return;
    }
    setBusy(true);
    clearLogs(instance.id);
    setRunning(instance.id, true);
    navigate(`/instances/${instance.id}/launch`);
    try {
      await launchApi.launch(instance.id);
    } catch (e) {
      setRunning(instance.id, false);
      toast.error(errorMessage(e));
    } finally {
      setBusy(false);
    }
  }

  async function stop() {
    try {
      await launchApi.stop(instance.id);
    } catch (e) {
      toast.error(errorMessage(e));
    }
  }

  const openDetail = () => navigate(running ? `/instances/${instance.id}/launch` : `/instances/${instance.id}`);

  return (
    <Card className={cn(installing && "opacity-60")}>
      <CardHeader
        role={installing ? undefined : "button"}
        tabIndex={installing ? undefined : 0}
        title={installing ? undefined : running ? "Voir les logs" : "Paramètres de l'instance"}
        onClick={installing ? undefined : openDetail}
        onKeyDown={
          installing ? undefined : (e) => (e.key === "Enter" || e.key === " ") && (e.preventDefault(), openDetail())
        }
        className={cn(
          "flex-row items-center gap-3 space-y-0 rounded-t-xl transition-colors",
          !installing && "cursor-pointer hover:bg-muted/50",
        )}
      >
        {instance.icon_url ? (
          <img src={instance.icon_url} alt="" className="size-10 rounded-md object-cover" />
        ) : instance.loader === "vanilla" ? (
          <MinecraftGrassIcon className="size-10 shrink-0 rounded-md" />
        ) : (
          <div className="flex size-10 shrink-0 items-center justify-center rounded-md bg-muted">
            <Blocks className="size-5 text-muted-foreground" aria-hidden="true" />
          </div>
        )}
        <div className="min-w-0 flex-1">
          <CardTitle className="truncate">{instance.name}</CardTitle>
          {installing ? (
            <div className="space-y-1 pt-0.5">
              <p className="text-xs text-muted-foreground">Installation… {installPercent}%</p>
              <Progress value={installPercent} className="h-1" />
            </div>
          ) : (
            <CardDescription className="truncate">
              {instance.minecraft_version}
              {instance.loader !== "vanilla" &&
                ` · ${instance.loader}${instance.loader_version ? ` ${instance.loader_version}` : ""}`}
            </CardDescription>
          )}
        </div>
      </CardHeader>
      <CardFooter className="justify-between gap-2">
        <div className="flex gap-1">
          <Button
            variant="ghost"
            size="icon-sm"
            title="Paramètres de l'instance"
            disabled={installing}
            onClick={() => navigate(`/instances/${instance.id}`)}
          >
            <Settings2 className="size-4" aria-hidden="true" />
          </Button>
          <Button
            variant="ghost"
            size="icon-sm"
            title="Ouvrir le dossier"
            disabled={installing}
            onClick={() => instancesApi.openFolder(instance.id)}
          >
            <FolderOpen className="size-4" aria-hidden="true" />
          </Button>
          <Button
            variant="ghost"
            size="icon-sm"
            title="Supprimer"
            disabled={running || installing}
            onClick={() => deleteMutation.mutate()}
          >
            <Trash2 className="size-4" aria-hidden="true" />
          </Button>
        </div>

        {running ? (
          <Button variant="destructive" size="sm" onClick={stop} className="gap-1.5">
            <Square className="size-3.5" aria-hidden="true" />
            Arrêter
          </Button>
        ) : (
          <Button size="sm" onClick={play} disabled={busy || installing} className="gap-1.5">
            {busy ? (
              <Loader2 className="size-3.5 animate-spin" aria-hidden="true" />
            ) : (
              <Play className="size-3.5" aria-hidden="true" />
            )}
            Jouer
          </Button>
        )}
      </CardFooter>
    </Card>
  );
}
