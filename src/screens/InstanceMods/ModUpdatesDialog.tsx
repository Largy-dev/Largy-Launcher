import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { ArrowRight, Loader2, RefreshCw } from "lucide-react";

import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Switch } from "@/components/ui/switch";
import { notify } from "@/lib/notify";
import { errorMessage, instanceModsApi, type ModUpdate } from "@/services/tauri";

interface ModUpdatesDialogProps {
  instanceId: string;
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

/** Lists mods Modrinth has a newer compatible version of, and updates the selected ones. */
export function ModUpdatesDialog({ instanceId, open, onOpenChange }: ModUpdatesDialogProps) {
  const queryClient = useQueryClient();
  const [skipped, setSkipped] = useState<Set<string>>(new Set());

  const updates = useQuery({
    queryKey: ["mod-updates", instanceId],
    queryFn: () => instanceModsApi.checkUpdates(instanceId),
    enabled: open,
    staleTime: 0,
  });
  const selected = (updates.data ?? []).filter((u) => !skipped.has(u.file_name));

  const apply = useMutation({
    mutationFn: (list: ModUpdate[]) => instanceModsApi.applyUpdates(instanceId, list),
    onSuccess: (failed, list) => {
      queryClient.invalidateQueries({ queryKey: ["instance-mods", instanceId] });
      queryClient.invalidateQueries({ queryKey: ["mod-updates", instanceId] });
      if (failed.length === 0) {
        notify.success({ title: `${list.length} mod${list.length > 1 ? "s" : ""} mis à jour` });
        onOpenChange(false);
      } else {
        notify.warning({ title: `${failed.length} mise(s) à jour en échec`, message: failed.join("\n") });
      }
    },
    onError: (e) => notify.error({ title: "Mise à jour impossible", message: errorMessage(e) }),
  });

  const toggle = (fileName: string, on: boolean) =>
    setSkipped((prev) => {
      const next = new Set(prev);
      if (on) next.delete(fileName);
      else next.add(fileName);
      return next;
    });

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="flex max-h-[85vh] flex-col sm:max-w-2xl">
        <DialogHeader>
          <DialogTitle>Mises à jour des mods</DialogTitle>
          <DialogDescription>Mods reconnus sur Modrinth avec une version plus récente compatible.</DialogDescription>
        </DialogHeader>

        <div className="-mx-1 min-h-0 flex-1 overflow-y-auto px-1">
          {updates.isFetching && (
            <div className="flex items-center justify-center gap-2 py-12 text-sm text-muted-foreground">
              <Loader2 className="size-4 animate-spin" aria-hidden="true" />
              Analyse des mods…
            </div>
          )}
          {updates.isError && (
            <p className="py-8 text-center text-sm text-destructive">{errorMessage(updates.error)}</p>
          )}
          {!updates.isFetching && updates.data?.length === 0 && (
            <p className="py-8 text-center text-sm text-muted-foreground">Tous les mods reconnus sont à jour.</p>
          )}
          <ul className="divide-y divide-border/60">
            {!updates.isFetching &&
              updates.data?.map((u) => (
                <li key={u.file_name} className="flex items-center gap-3 py-2.5">
                  {u.icon_url ? (
                    <img src={u.icon_url} alt="" className="size-9 shrink-0 rounded-lg object-cover" />
                  ) : (
                    <div className="size-9 shrink-0 rounded-lg bg-muted" />
                  )}
                  <div className="min-w-0 flex-1">
                    <p className="truncate text-sm font-medium">{u.title || u.file_name}</p>
                    <p className="flex items-center gap-1.5 truncate text-xs text-muted-foreground">
                      {u.current_version}
                      <ArrowRight className="size-3" aria-hidden="true" />
                      <span className="text-foreground">{u.new_version}</span>
                    </p>
                  </div>
                  <Switch
                    checked={!skipped.has(u.file_name)}
                    aria-label={`Mettre à jour ${u.title}`}
                    onCheckedChange={(on) => toggle(u.file_name, on)}
                  />
                </li>
              ))}
          </ul>
        </div>

        <DialogFooter>
          <Button variant="outline" onClick={() => updates.refetch()} disabled={updates.isFetching} className="gap-1.5">
            <RefreshCw aria-hidden="true" />
            Revérifier
          </Button>
          <Button
            disabled={selected.length === 0 || apply.isPending || updates.isFetching}
            onClick={() => apply.mutate(selected)}
            className="gap-1.5"
          >
            {apply.isPending && <Loader2 className="animate-spin" aria-hidden="true" />}
            Mettre à jour ({selected.length})
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
