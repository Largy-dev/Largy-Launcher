import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";
import { AlertTriangle, Check, Gauge, Loader2, Minus } from "lucide-react";

import type { OptimizeItem } from "@/bindings/OptimizeItem";
import type { OptimizeResult } from "@/bindings/OptimizeResult";
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
import { notify } from "@/lib/notify";
import { errorMessage, type Instance } from "@/services/tauri";

interface OptimizeDialogProps {
  instance: Instance;
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

/** One click for the classic performance mods that fit this instance. */
export function OptimizeDialog({ instance, open, onOpenChange }: OptimizeDialogProps) {
  const queryClient = useQueryClient();
  const [skipped, setSkipped] = useState<Set<string>>(new Set());

  const plan = useQuery({
    queryKey: ["optimize-plan", instance.id],
    queryFn: () => invoke<OptimizeItem[]>("instance_optimize_plan", { instanceId: instance.id }),
    enabled: open,
    staleTime: 0,
  });
  const installable = (plan.data ?? []).filter((i) => i.status === "install");
  const chosen = installable.filter((i) => !skipped.has(i.slug));

  const apply = useMutation({
    mutationFn: (slugs: string[]) =>
      invoke<OptimizeResult>("instance_optimize_apply", { instanceId: instance.id, slugs }),
    onSuccess: (result) => {
      queryClient.invalidateQueries({ queryKey: ["installed", instance.id] });
      queryClient.invalidateQueries({ queryKey: ["content-summary", instance.id] });
      queryClient.invalidateQueries({ queryKey: ["snapshots", instance.id] });
      queryClient.invalidateQueries({ queryKey: ["optimize-plan", instance.id] });
      if (result.failed.length === 0) {
        notify.success({
          title: "Instance optimisée",
          message: `${result.installed.length} fichier${result.installed.length > 1 ? "s" : ""} installé${result.installed.length > 1 ? "s" : ""}. Un point de restauration a été créé avant.`,
        });
        onOpenChange(false);
      } else {
        notify.warning({ title: "Optimisation incomplète", message: result.failed.join("\n") });
      }
    },
    onError: (e) => notify.error({ title: "Optimisation impossible", message: errorMessage(e) }),
  });

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="flex max-h-[85vh] flex-col sm:max-w-xl">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <Gauge className="size-5 text-primary" aria-hidden="true" />
            Optimiser les performances
          </DialogTitle>
          <DialogDescription>
            Les mods de performance reconnus, dans leur version pour Minecraft {instance.minecraft_version}. Rien ne
            change dans le gameplay.
          </DialogDescription>
        </DialogHeader>

        {instance.modpack && (
          <p className="flex items-start gap-2 rounded-lg border border-amber-500/30 bg-amber-500/10 px-3 py-2 text-xs">
            <AlertTriangle className="mt-px size-3.5 shrink-0 text-amber-500" aria-hidden="true" />
            Cette instance est un modpack : son auteur l'a déjà réglé. Ajouter des mods peut créer des conflits — un
            point de restauration est créé avant, au cas où.
          </p>
        )}

        <div className="-mx-1 min-h-0 flex-1 overflow-x-hidden overflow-y-auto px-1">
          {plan.isFetching && (
            <div className="flex items-center justify-center gap-2 py-12 text-sm text-muted-foreground">
              <Loader2 className="size-4 animate-spin" aria-hidden="true" />
              Recherche des versions compatibles…
            </div>
          )}
          {plan.isError && <p className="py-8 text-center text-sm text-destructive">{errorMessage(plan.error)}</p>}
          {!plan.isFetching && (
            <ul className="divide-y divide-border/60">
              {(plan.data ?? []).map((item) => (
                <li key={item.slug} className="flex items-center gap-3 py-2.5">
                  {item.icon_url ? (
                    <img src={item.icon_url} alt="" className="size-9 shrink-0 rounded-lg object-cover" />
                  ) : (
                    <div className="size-9 shrink-0 rounded-lg bg-muted" />
                  )}
                  <div className="min-w-0 flex-1">
                    <p className="truncate text-sm font-medium">{item.title}</p>
                    <p className="text-xs text-muted-foreground">
                      {item.status === "present"
                        ? `Déjà assuré par ${item.covered_by}`
                        : item.status === "unavailable"
                          ? "Pas encore disponible pour cette version"
                          : item.reason}
                    </p>
                  </div>
                  {item.status === "install" ? (
                    <Checkbox
                      checked={!skipped.has(item.slug)}
                      aria-label={`Installer ${item.title}`}
                      onCheckedChange={(on) =>
                        setSkipped((prev) => {
                          const next = new Set(prev);
                          if (on === true) next.delete(item.slug);
                          else next.add(item.slug);
                          return next;
                        })
                      }
                    />
                  ) : item.status === "present" ? (
                    <Check className="size-4 text-primary" aria-label="Déjà présent" />
                  ) : (
                    <Minus className="size-4 text-muted-foreground" aria-label="Indisponible" />
                  )}
                </li>
              ))}
            </ul>
          )}
          {!plan.isFetching && plan.isSuccess && installable.length === 0 && (
            <p className="py-4 text-center text-sm text-muted-foreground">
              Rien à ajouter : cette instance est déjà optimisée.
            </p>
          )}
        </div>

        <DialogFooter>
          <Button variant="outline" onClick={() => onOpenChange(false)}>
            Fermer
          </Button>
          <Button
            disabled={chosen.length === 0 || apply.isPending || plan.isFetching}
            onClick={() => apply.mutate(chosen.map((i) => i.slug))}
            className="gap-1.5"
          >
            {apply.isPending && <Loader2 className="animate-spin" aria-hidden="true" />}
            Installer ({chosen.length})
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
