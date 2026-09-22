import { openUrl } from "@tauri-apps/plugin-opener";
import { Copy, ExternalLink, FolderOpen } from "lucide-react";

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

export function InstallWarningsDialog() {
  const pending = useAppStore((s) => s.installWarnings);
  const clear = useAppStore((s) => s.clearInstallWarnings);

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
              `${pending.instanceName} — ${pending.warnings.length} fichier(s) n'ont pas pu être téléchargés automatiquement. Télécharge-les depuis la page CurseForge puis dépose-les dans le dossier de l'instance.`}
          </DialogDescription>
        </DialogHeader>

        <div className="max-h-80 space-y-2 overflow-y-auto">
          {pending?.warnings.map((warning, i) => (
            <div key={i} className="space-y-1 rounded-md border border-border p-3">
              <p className="truncate text-sm font-medium">{warning.file_name}</p>
              <p className="text-xs text-muted-foreground">{warning.message}</p>
              {warning.browser_url && (
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
          ))}
        </div>

        <DialogFooter className="flex-col gap-2 sm:flex-row sm:justify-between">
          <div className="flex gap-2">
            <Button
              variant="outline"
              size="sm"
              className="gap-1.5"
              onClick={() => pending && instancesApi.openFolder(pending.instanceId)}
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
