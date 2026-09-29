import { useMemo, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Check, Globe, Loader2, Puzzle, Search } from "lucide-react";

import { LoaderBadge } from "@/components/instance/LoaderBadge";
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
import { Switch } from "@/components/ui/switch";
import { formatRelative } from "@/lib/format";
import { notify } from "@/lib/notify";
import { EXTERNAL_SOURCE_LABEL, externalApi, type ExternalInstance } from "@/services/external";
import { errorMessage } from "@/services/tauri";

interface ExternalImportDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

/** Brings instances over from the official launcher, Prism, CurseForge and the Modrinth App. */
export function ExternalImportDialog({ open, onOpenChange }: ExternalImportDialogProps) {
  const queryClient = useQueryClient();
  const [picked, setPicked] = useState<Set<string>>(new Set());
  const [withWorlds, setWithWorlds] = useState(true);
  const [done, setDone] = useState<Set<string>>(new Set());

  const found = useQuery({ queryKey: ["external-instances"], queryFn: externalApi.detect, enabled: open });
  const importable = useMemo(
    () => (found.data ?? []).filter((e) => !e.already_imported && !done.has(e.id)),
    [found.data, done],
  );

  const run = useMutation({
    mutationFn: async (list: ExternalInstance[]) => {
      const failed: string[] = [];
      for (const external of list) {
        try {
          await externalApi.import(external.id, withWorlds);
          setDone((prev) => new Set(prev).add(external.id));
        } catch (e) {
          failed.push(`${external.name} : ${errorMessage(e)}`);
        }
      }
      return { imported: list.length - failed.length, failed };
    },
    onSuccess: ({ imported, failed }) => {
      queryClient.invalidateQueries({ queryKey: ["instances"] });
      queryClient.invalidateQueries({ queryKey: ["external-instances"] });
      setPicked(new Set());
      if (imported > 0) {
        notify.success({
          title: `${imported} instance${imported > 1 ? "s" : ""} importée${imported > 1 ? "s" : ""}`,
          message: "Le jeu et le mod loader s'installeront au premier lancement.",
        });
      }
      if (failed.length > 0) notify.warning({ title: "Import incomplet", message: failed.join("\n") });
      if (failed.length === 0) onOpenChange(false);
    },
  });

  const toggle = (id: string, on: boolean) =>
    setPicked((prev) => {
      const next = new Set(prev);
      if (on) next.add(id);
      else next.delete(id);
      return next;
    });

  const chosen = importable.filter((e) => picked.has(e.id));

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="flex max-h-[85vh] flex-col sm:max-w-2xl">
        <DialogHeader>
          <DialogTitle>Importer depuis un autre launcher</DialogTitle>
          <DialogDescription>
            Tes instances du launcher officiel, de Prism / MultiMC, de CurseForge et de la Modrinth App : mods, configs,
            packs et options sont copiés, l'original n'est pas modifié.
          </DialogDescription>
        </DialogHeader>

        <div className="-mx-1 min-h-0 flex-1 overflow-x-hidden overflow-y-auto px-1">
          {found.isLoading && (
            <div className="flex items-center justify-center gap-2 py-12 text-sm text-muted-foreground">
              <Loader2 className="size-4 animate-spin" aria-hidden="true" />
              Recherche des autres launchers…
            </div>
          )}
          {found.isError && <p className="py-8 text-center text-sm text-destructive">{errorMessage(found.error)}</p>}
          {found.isSuccess && found.data.length === 0 && (
            <div className="flex flex-col items-center gap-2 py-10 text-center text-sm text-muted-foreground">
              <Search className="size-6" aria-hidden="true" />
              Aucune instance trouvée dans les autres launchers de ce PC.
            </div>
          )}
          <ul className="divide-y divide-border/60">
            {(found.data ?? []).map((e) => {
              const imported = e.already_imported || done.has(e.id);
              return (
                <li key={e.id}>
                  <label
                    className="flex cursor-pointer items-center gap-3 py-2.5 aria-disabled:cursor-default aria-disabled:opacity-60"
                    aria-disabled={imported}
                  >
                    {imported ? (
                      <Check className="size-4 shrink-0 text-primary" aria-label="Déjà importée" />
                    ) : (
                      <Checkbox checked={picked.has(e.id)} onCheckedChange={(on) => toggle(e.id, on === true)} />
                    )}
                    <div className="min-w-0 flex-1">
                      <p className="flex min-w-0 items-center gap-2 text-sm font-medium">
                        <span className="truncate">{e.name}</span>
                        <LoaderBadge loader={e.loader} version={e.loader_version} />
                        <span className="shrink-0 text-xs font-normal text-muted-foreground">
                          {e.minecraft_version}
                        </span>
                      </p>
                      <p className="flex items-center gap-3 truncate text-xs text-muted-foreground">
                        <span>{EXTERNAL_SOURCE_LABEL[e.source]}</span>
                        {e.mods > 0 && (
                          <span className="flex items-center gap-1">
                            <Puzzle className="size-3" aria-hidden="true" />
                            {e.mods} mods
                          </span>
                        )}
                        {e.worlds > 0 && (
                          <span className="flex items-center gap-1">
                            <Globe className="size-3" aria-hidden="true" />
                            {e.worlds} monde{e.worlds > 1 ? "s" : ""}
                          </span>
                        )}
                        {e.last_played && <span>joué {formatRelative(e.last_played)}</span>}
                        {imported && <span className="text-primary">Déjà importée</span>}
                      </p>
                    </div>
                  </label>
                </li>
              );
            })}
          </ul>
        </div>

        <DialogFooter className="items-center sm:justify-between">
          <label className="flex items-center gap-2 text-sm">
            <Switch checked={withWorlds} onCheckedChange={setWithWorlds} />
            Copier aussi les mondes
          </label>
          <div className="flex gap-2">
            <Button variant="outline" onClick={() => onOpenChange(false)}>
              Fermer
            </Button>
            <Button
              disabled={chosen.length === 0 || run.isPending}
              onClick={() => run.mutate(chosen)}
              className="gap-1.5"
            >
              {run.isPending && <Loader2 className="animate-spin" aria-hidden="true" />}
              Importer ({chosen.length})
            </Button>
          </div>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
