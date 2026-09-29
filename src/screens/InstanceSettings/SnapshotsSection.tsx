import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { History, Loader2, Plus, RotateCcw, Trash2 } from "lucide-react";

import { ConfirmDialog } from "@/components/ConfirmDialog";
import { SettingSection } from "@/components/settings/SettingsKit";
import { LOADER_META } from "@/components/instance/LoaderBadge";
import { Button } from "@/components/ui/button";
import { formatRelative } from "@/lib/format";
import { notify } from "@/lib/notify";
import { snapshotsApi, type Snapshot } from "@/services/content";
import { errorMessage, type Instance } from "@/services/tauri";

/** Restore points taken before updates (or by hand), one click from undoing a broken update. */
export function SnapshotsSection({ instance }: { instance: Instance }) {
  const queryClient = useQueryClient();
  const queryKey = ["snapshots", instance.id];
  const { data: snapshots, isLoading } = useQuery({ queryKey, queryFn: () => snapshotsApi.list(instance.id) });
  const [toRestore, setToRestore] = useState<Snapshot | null>(null);

  const refresh = () => queryClient.invalidateQueries({ queryKey });

  const create = useMutation({
    mutationFn: () => snapshotsApi.create(instance.id),
    onSuccess: () => {
      refresh();
      notify.success({ title: "Point de restauration créé", history: false });
    },
    onError: (e) => notify.error({ title: "Création impossible", message: errorMessage(e) }),
  });

  const restore = useMutation({
    mutationFn: (snapshot: Snapshot) => snapshotsApi.restore(instance.id, snapshot.id),
    onSuccess: (restored) => {
      queryClient.setQueryData(["instance", instance.id], restored);
      queryClient.invalidateQueries({ queryKey: ["installed", instance.id] });
      queryClient.invalidateQueries({ queryKey: ["content-summary", instance.id] });
      queryClient.invalidateQueries({ queryKey: ["instances"] });
      refresh();
      setToRestore(null);
      notify.success({
        title: "Instance restaurée",
        message: "L'état d'avant la restauration a lui aussi été gardé, au cas où.",
      });
    },
    onError: (e) => notify.error({ title: "Restauration impossible", message: errorMessage(e) }),
  });

  const remove = useMutation({
    mutationFn: (snapshot: Snapshot) => snapshotsApi.delete(instance.id, snapshot.id),
    onSuccess: refresh,
    onError: (e) => notify.error({ title: "Suppression impossible", message: errorMessage(e), history: false }),
  });

  return (
    <SettingSection title="Points de restauration">
      <div className="flex items-center gap-3 px-4 py-3">
        <p className="flex-1 text-sm text-muted-foreground">
          Créés automatiquement avant chaque mise à jour de mods ou du modpack : mods, configs et version reviennent en
          un clic. Les mondes ont leurs propres sauvegardes.
        </p>
        <Button
          variant="outline"
          size="sm"
          className="shrink-0 gap-1.5"
          disabled={create.isPending}
          onClick={() => create.mutate()}
        >
          {create.isPending ? <Loader2 className="animate-spin" aria-hidden="true" /> : <Plus aria-hidden="true" />}
          Créer maintenant
        </Button>
      </div>
      {isLoading ? null : !snapshots?.length ? (
        <p className="flex items-center gap-2 border-t border-border/60 px-4 py-3 text-sm text-muted-foreground">
          <History className="size-4" aria-hidden="true" />
          Aucun point de restauration pour l'instant.
        </p>
      ) : (
        <ul className="divide-y divide-border/60 border-t border-border/60">
          {snapshots.map((s) => (
            <li key={s.id} className="flex items-center gap-3 px-4 py-2.5">
              <div className="min-w-0 flex-1">
                <p className="truncate text-sm font-medium">{s.reason}</p>
                <p className="truncate text-xs text-muted-foreground">
                  {formatRelative(s.created_at)} · Minecraft {s.minecraft_version} · {LOADER_META[s.loader].label}
                  {s.loader_version ? ` ${s.loader_version}` : ""} · {s.mods} mod{s.mods > 1 ? "s" : ""}
                </p>
              </div>
              <Button variant="outline" size="sm" className="gap-1.5" onClick={() => setToRestore(s)}>
                <RotateCcw aria-hidden="true" />
                Restaurer
              </Button>
              <Button
                variant="ghost"
                size="icon-sm"
                aria-label="Supprimer ce point de restauration"
                disabled={remove.isPending}
                onClick={() => remove.mutate(s)}
              >
                <Trash2 aria-hidden="true" />
              </Button>
            </li>
          ))}
        </ul>
      )}
      <ConfirmDialog
        open={toRestore !== null}
        onOpenChange={(o) => !o && setToRestore(null)}
        title="Restaurer ce point ?"
        description={
          toRestore
            ? `Mods, resource packs, shaders et configs reviennent à leur état « ${toRestore.reason} » (${formatRelative(toRestore.created_at)}). L'état actuel est gardé comme nouveau point de restauration.`
            : ""
        }
        confirmLabel="Restaurer"
        pending={restore.isPending}
        onConfirm={() => toRestore && restore.mutate(toRestore)}
      />
    </SettingSection>
  );
}
