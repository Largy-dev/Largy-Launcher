import { useEffect, useMemo, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { openUrl } from "@tauri-apps/plugin-opener";
import { CheckCircle2, Copy, ExternalLink, FolderOpen, Loader2 } from "lucide-react";

import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { notify } from "@/lib/notify";
import { instancesApi } from "@/services/tauri";
import { useAppStore } from "@/store/appStore";

/** How often the Downloads folder is checked for hand-downloaded files. */
const COLLECT_INTERVAL_MS = 2000;

export function InstallWarningsDialog() {
  const pending = useAppStore((s) => s.installWarnings);
  const clear = useAppStore((s) => s.clearInstallWarnings);
  const queryClient = useQueryClient();
  // Tagged with the list it belongs to, so a new install's list starts empty.
  const [collectedFor, setCollectedFor] = useState<{ owner: typeof pending; paths: Set<string> } | null>(null);
  const collected = useMemo(
    () => (collectedFor && collectedFor.owner === pending ? collectedFor.paths : new Set<string>()),
    [collectedFor, pending],
  );

  const manual = useMemo(
    () => (pending?.warnings ?? []).filter((w) => w.path).map((w) => ({ path: w.path!, sha1: w.sha1 ?? null })),
    [pending],
  );
  const remaining = manual.filter((f) => !collected.has(f.path));

  // Files the player downloads from the listed pages land in Downloads:
  // move them into the instance as soon as they show up. Each check waits
  // for the previous one (hashing big jars can take a moment).
  useEffect(() => {
    const wanted = manual.filter((f) => !collected.has(f.path));
    if (!pending || wanted.length === 0) return;
    let stopped = false;
    let timer: number | undefined;
    const tick = async () => {
      try {
        const placed = await instancesApi.collectManualDownloads(pending.instanceId, wanted);
        if (stopped) return;
        if (placed.length > 0) {
          const next = new Set([...collected, ...placed]);
          setCollectedFor({ owner: pending, paths: next });
          queryClient.invalidateQueries({ queryKey: ["instance-mods", pending.instanceId] });
          if (manual.every((f) => next.has(f.path))) {
            notify.success({
              title: `${pending.instanceName} est complet`,
              message: "Tous les fichiers sont en place.",
            });
          }
          return;
        }
      } catch {
        // Downloads folder unavailable: the manual "open folder" route still works.
      }
      if (!stopped) timer = window.setTimeout(tick, COLLECT_INTERVAL_MS);
    };
    void tick();
    return () => {
      stopped = true;
      window.clearTimeout(timer);
    };
  }, [pending, manual, collected, queryClient]);

  async function copyAll() {
    if (!pending) return;
    const text = pending.warnings
      .map((w) => `${w.file_name} — ${w.message}${w.browser_url ? ` (${w.browser_url})` : ""}`)
      .join("\n");
    try {
      await navigator.clipboard.writeText(text);
      notify.success({ title: "Liste copiée", history: false });
    } catch {
      notify.error({ title: "Impossible de copier dans le presse-papiers", history: false });
    }
  }

  return (
    <Dialog open={pending !== null} onOpenChange={(open) => !open && clear()}>
      <DialogContent className="sm:max-w-lg">
        <DialogHeader>
          <DialogTitle>Installation incomplète</DialogTitle>
          <DialogDescription>
            {pending &&
              (manual.length > 0
                ? `${pending.instanceName} — ${pending.warnings.length} fichier(s) n'ont pas pu être téléchargés automatiquement. Ouvre leur page et télécharge-les : le launcher les récupère tout seul dans ton dossier Téléchargements, garde cette fenêtre ouverte.`
                : `${pending.instanceName} — ${pending.warnings.length} fichier(s) n'ont pas pu être téléchargés automatiquement. Télécharge-les depuis leur page puis dépose-les dans le dossier de l'instance.`)}
          </DialogDescription>
        </DialogHeader>

        {manual.length > 0 && (
          <p className="flex items-center gap-2 text-xs text-muted-foreground">
            {remaining.length > 0 ? (
              <Loader2 className="size-3.5 animate-spin" aria-hidden="true" />
            ) : (
              <CheckCircle2 className="size-3.5 text-emerald-500" aria-hidden="true" />
            )}
            {manual.length - remaining.length} / {manual.length} fichier(s) récupéré(s)
          </p>
        )}

        <div className="max-h-80 space-y-2 overflow-y-auto">
          {pending?.warnings.map((warning, i) => {
            const done = !!warning.path && collected.has(warning.path);
            return (
              <div key={i} className="space-y-1 rounded-md border border-border p-3">
                <p className="flex items-center gap-1.5 truncate text-sm font-medium">
                  {done && <CheckCircle2 className="size-3.5 shrink-0 text-emerald-500" aria-label="Récupéré" />}
                  {warning.file_name}
                </p>
                {!done && <p className="text-xs text-muted-foreground">{warning.message}</p>}
                {warning.browser_url && !done && (
                  <Button
                    variant="outline"
                    size="sm"
                    className="mt-1 gap-1.5"
                    onClick={() => openUrl(warning.browser_url!)}
                  >
                    <ExternalLink className="size-3.5" aria-hidden="true" />
                    Ouvrir la page de téléchargement
                  </Button>
                )}
              </div>
            );
          })}
        </div>

        <DialogFooter className="flex-col gap-2 sm:flex-row sm:justify-between">
          <div className="flex gap-2">
            <Button
              variant="outline"
              size="sm"
              className="gap-1.5"
              onClick={() => pending && instancesApi.openFolder(pending.instanceId, "mods")}
            >
              <FolderOpen className="size-3.5" aria-hidden="true" />
              Ouvrir le dossier de l'instance
            </Button>
            <Button variant="outline" size="sm" className="gap-1.5" onClick={copyAll}>
              <Copy className="size-3.5" aria-hidden="true" />
              Copier la liste
            </Button>
          </div>
          <Button size="sm" onClick={clear}>
            Fermer
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
