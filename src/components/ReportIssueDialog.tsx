import { useState } from "react";
import { useMutation } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import { Bug, Loader2 } from "lucide-react";

import type { LogSource } from "@/bindings/LogSource";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Textarea } from "@/components/ui/textarea";
import { useModCount } from "@/hooks/useInstanceInfo";
import { buildIssueUrl } from "@/lib/issueReport";
import { notify } from "@/lib/notify";
import { errorMessage, getAppVersion, type Instance } from "@/services/tauri";

interface ReportIssueDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  /** The instance the problem is about, if any. */
  instance?: Instance;
  crashSummary?: string | null;
  /** The crash report of this very crash (path), when the game wrote one. */
  crashReport?: string | null;
}

/**
 * Opens a prefilled GitHub issue: the player's description, versions, and —
 * if they agree — links to their logs (uploaded to mclo.gs, scrubbed of
 * personal details). Nothing is sent anywhere until they click.
 */
export function ReportIssueDialog({ open, onOpenChange, instance, crashSummary, crashReport }: ReportIssueDialogProps) {
  const [description, setDescription] = useState("");
  const [launcherLog, setLauncherLog] = useState(true);
  const [gameLog, setGameLog] = useState(true);
  const mods = useModCount(instance);

  const report = useMutation({
    mutationFn: async () => {
      const [appVersion, os] = await Promise.all([getAppVersion(), invoke<string>("system_os_version")]);
      // This crash's own report, else the game log — never an older crash's report.
      const logs: { source: LogSource; crashReport: string | null }[] = [];
      if (launcherLog) logs.push({ source: "launcher", crashReport: null });
      if (instance && gameLog) {
        logs.push(crashReport ? { source: "crash", crashReport } : { source: "game", crashReport: null });
      }
      const logUrls: string[] = [];
      for (const log of logs) {
        try {
          logUrls.push(await invoke<string>("logs_share", { ...log, instanceId: instance?.id ?? null }));
        } catch {
          // Nothing to attach (no log yet, or mclo.gs unreachable): the report goes without it.
        }
      }
      return buildIssueUrl({
        appVersion,
        os,
        description,
        crashSummary,
        logUrls,
        instance: instance && {
          name: instance.name,
          minecraftVersion: instance.minecraft_version,
          loader: instance.loader,
          loaderVersion: instance.loader_version,
          mods,
        },
      });
    },
    onSuccess: async (url) => {
      await openUrl(url);
      notify.info({
        title: "Page GitHub ouverte",
        message: "Vérifie le texte puis clique sur « Create » (un compte GitHub gratuit est nécessaire). Merci !",
      });
      onOpenChange(false);
      setDescription("");
    },
    onError: (e) => notify.error({ title: "Signalement impossible", message: errorMessage(e) }),
  });

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-lg">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <Bug className="size-5 text-primary" aria-hidden="true" />
            Signaler un problème
          </DialogTitle>
          <DialogDescription>
            Un rapport pré-rempli s'ouvre sur GitHub : tu le relis avant de l'envoyer. Rien n'est envoyé sans toi.
          </DialogDescription>
        </DialogHeader>
        <Textarea
          autoFocus
          className="min-h-28"
          maxLength={3000}
          placeholder="Que s'est-il passé ? Qu'est-ce que tu faisais juste avant ?"
          value={description}
          onChange={(e) => setDescription(e.target.value)}
        />
        <div className="space-y-2 text-sm">
          <label className="flex items-center gap-2">
            <Checkbox checked={launcherLog} onCheckedChange={(v) => setLauncherLog(v === true)} />
            Joindre le journal du launcher
          </label>
          {instance && (
            <label className="flex items-center gap-2">
              <Checkbox checked={gameLog} onCheckedChange={(v) => setGameLog(v === true)} />
              Joindre le {crashReport ? "rapport de crash" : "log du jeu"} de « {instance.name} »
            </label>
          )}
          <p className="text-xs text-muted-foreground">
            Les logs sont publiés sur mclo.gs (lien public), sans ton nom de session Windows ni tes jetons.
          </p>
        </div>
        <DialogFooter>
          <Button variant="outline" onClick={() => onOpenChange(false)}>
            Annuler
          </Button>
          <Button onClick={() => report.mutate()} disabled={report.isPending} className="gap-1.5">
            {report.isPending && <Loader2 className="animate-spin" aria-hidden="true" />}
            Préparer le rapport
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
