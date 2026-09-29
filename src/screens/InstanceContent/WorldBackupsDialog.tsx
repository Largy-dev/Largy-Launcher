import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Archive, ArchiveRestore, FolderOpen, Layers, Loader2, Trash2 } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { formatBytes, formatRelative } from "@/lib/format";
import { notify } from "@/lib/notify";
import { worldsApi, type WorldBackup } from "@/services/content";
import { errorMessage, instancesApi, type Instance } from "@/services/tauri";

interface WorldBackupsDialogProps {
  instance: Instance;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onRestored: () => void;
}

/** Every world backup of an instance — per world or all at once — restorable side by side. */
export function WorldBackupsDialog({ instance, open, onOpenChange, onRestored }: WorldBackupsDialogProps) {
  const queryClient = useQueryClient();
  const queryKey = ["world-backups", instance.id];
  const backups = useQuery({ queryKey, queryFn: () => worldsApi.backups(instance.id), enabled: open });

  const restore = useMutation({
    mutationFn: (backup: WorldBackup) => worldsApi.restore(instance.id, backup.id),
    onSuccess: (created) => {
      onRestored();
      notify.success({
        title: created.length > 1 ? `${created.length} mondes restaurés` : "Monde restauré",
        message: `Ajouté${created.length > 1 ? "s" : ""} à côté de tes mondes : ${created.join(", ")}`,
      });
    },
    onError: (e) => notify.error({ title: "Restauration impossible", message: errorMessage(e) }),
  });

  const remove = useMutation({
    mutationFn: (backup: WorldBackup) => worldsApi.deleteBackup(instance.id, backup.id),
    onSuccess: () => queryClient.invalidateQueries({ queryKey }),
    onError: (e) => notify.error({ title: "Suppression impossible", message: errorMessage(e), history: false }),
  });

  const list = backups.data ?? [];

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="flex max-h-[80vh] flex-col sm:max-w-xl">
        <DialogHeader>
          <DialogTitle>Sauvegardes des mondes</DialogTitle>
          <DialogDescription>
            Une restauration n'écrase jamais un monde : elle l'ajoute à côté, sous un autre nom si besoin.
          </DialogDescription>
        </DialogHeader>
        <div className="-mx-1 min-h-0 flex-1 overflow-y-auto px-1">
          {backups.isLoading && (
            <div className="flex justify-center py-10">
              <Loader2 className="size-5 animate-spin text-muted-foreground" aria-hidden="true" />
            </div>
          )}
          {backups.isSuccess && list.length === 0 && (
            <p className="py-10 text-center text-sm text-muted-foreground">
              Aucune sauvegarde pour l'instant. Sauvegarde un monde depuis son menu « … ».
            </p>
          )}
          <ul className="divide-y divide-border/60">
            {list.map((b) => (
              <li key={b.id} className="flex items-center gap-3 py-2.5">
                <div className="flex size-9 shrink-0 items-center justify-center rounded-lg bg-primary/10 text-primary">
                  {b.world ? (
                    <Archive className="size-4" aria-hidden="true" />
                  ) : (
                    <Layers className="size-4" aria-hidden="true" />
                  )}
                </div>
                <div className="min-w-0 flex-1">
                  <p className="truncate text-sm font-medium">{b.world ?? "Tous les mondes"}</p>
                  <p className="text-xs text-muted-foreground">
                    {formatRelative(b.created_at)} · {formatBytes(b.size)}
                  </p>
                </div>
                <Button
                  size="sm"
                  variant="outline"
                  className="gap-1.5"
                  disabled={restore.isPending}
                  onClick={() => restore.mutate(b)}
                >
                  {restore.isPending && restore.variables?.id === b.id ? (
                    <Loader2 className="animate-spin" aria-hidden="true" />
                  ) : (
                    <ArchiveRestore aria-hidden="true" />
                  )}
                  Restaurer
                </Button>
                <Button
                  size="icon-sm"
                  variant="ghost"
                  aria-label="Supprimer cette sauvegarde"
                  disabled={remove.isPending}
                  onClick={() => remove.mutate(b)}
                >
                  <Trash2 aria-hidden="true" />
                </Button>
              </li>
            ))}
          </ul>
        </div>
        <Button
          variant="ghost"
          size="sm"
          className="gap-1.5 self-start"
          onClick={() => instancesApi.openFolder(instance.id, "backups")}
        >
          <FolderOpen aria-hidden="true" />
          Ouvrir le dossier des sauvegardes
        </Button>
      </DialogContent>
    </Dialog>
  );
}
