import { useState } from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { useNavigate } from "react-router";
import { Blocks, FolderOpen, Loader2, Play, Settings2, Square, Trash2 } from "lucide-react";
import { toast } from "sonner";

import { Button } from "@/components/ui/button";
import { Card, CardDescription, CardFooter, CardHeader, CardTitle } from "@/components/ui/card";
import { errorMessage, instancesApi, launchApi, type Instance } from "@/services/tauri";
import { useAppStore } from "@/store/appStore";

export function InstanceCard({ instance }: { instance: Instance }) {
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const account = useAppStore((s) => s.account);
  const running = useAppStore((s) => s.runtime[instance.id]?.running ?? false);
  const setRunning = useAppStore((s) => s.setRunning);
  const clearLogs = useAppStore((s) => s.clearLogs);
  const [busy, setBusy] = useState(false);

  const deleteMutation = useMutation({
    mutationFn: () => instancesApi.delete(instance.id),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ["instances"] }),
    onError: (e) => toast.error(errorMessage(e)),
  });

  async function play() {
    if (!account) {
      toast.error("Connecte-toi avec ton compte Microsoft avant de jouer.");
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

  return (
    <Card>
      <CardHeader className="flex-row items-center gap-3 space-y-0">
        {instance.icon_url ? (
          <img src={instance.icon_url} alt="" className="size-10 rounded-md object-cover" />
        ) : (
          <div className="flex size-10 shrink-0 items-center justify-center rounded-md bg-muted">
            <Blocks className="size-5 text-muted-foreground" aria-hidden="true" />
          </div>
        )}
        <div className="min-w-0 flex-1">
          <CardTitle className="truncate">{instance.name}</CardTitle>
          <CardDescription className="truncate">
            {instance.minecraft_version}
            {instance.loader !== "vanilla" &&
              ` · ${instance.loader}${instance.loader_version ? ` ${instance.loader_version}` : ""}`}
          </CardDescription>
        </div>
      </CardHeader>
      <CardFooter className="justify-between gap-2">
        <div className="flex gap-1">
          <Button
            variant="ghost"
            size="icon-sm"
            title="Paramètres de l'instance"
            onClick={() => navigate(`/instances/${instance.id}`)}
          >
            <Settings2 className="size-4" aria-hidden="true" />
          </Button>
          <Button
            variant="ghost"
            size="icon-sm"
            title="Ouvrir le dossier"
            onClick={() => instancesApi.openFolder(instance.id)}
          >
            <FolderOpen className="size-4" aria-hidden="true" />
          </Button>
          <Button
            variant="ghost"
            size="icon-sm"
            title="Supprimer"
            disabled={running}
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
          <Button size="sm" onClick={play} disabled={busy} className="gap-1.5">
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
