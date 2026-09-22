import { useMemo, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useNavigate, useParams } from "react-router";
import { open } from "@tauri-apps/plugin-dialog";
import { motion } from "motion/react";
import { ArrowLeft, Blocks, FolderOpen, Loader2, Plus, Puzzle, Search, Trash2 } from "lucide-react";

import { ConfirmDialog } from "@/components/ConfirmDialog";
import { EmptyState } from "@/components/EmptyState";
import { PageHeader } from "@/components/PageHeader";
import { Skeleton } from "@/components/Skeleton";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Switch } from "@/components/ui/switch";
import { formatBytes } from "@/lib/format";
import { listItem } from "@/lib/motion";
import { notify } from "@/lib/notify";
import { cn } from "@/lib/utils";
import { errorMessage, instanceModsApi, instancesApi, type ModEntry } from "@/services/tauri";

/** "create-1.20.1-0.5.1f.jar" → "create 1.20.1 0.5.1f" — easier to scan than raw file names. */
function prettyModName(fileName: string): string {
  return fileName
    .replace(/\.jar(\.disabled)?$/i, "")
    .replace(/[-_+]/g, " ")
    .trim();
}

type Filter = "all" | "enabled" | "disabled";

export function InstanceModsScreen() {
  const { id } = useParams<{ id: string }>();
  const instanceId = id ?? "";
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const [search, setSearch] = useState("");
  const [filter, setFilter] = useState<Filter>("all");
  const [toDelete, setToDelete] = useState<ModEntry | null>(null);

  const { data: instance } = useQuery({
    queryKey: ["instance", instanceId],
    queryFn: () => instancesApi.get(instanceId),
    enabled: instanceId !== "",
  });

  const modsQuery = useQuery({
    queryKey: ["instance-mods", instanceId],
    queryFn: () => instanceModsApi.list(instanceId),
    enabled: instanceId !== "",
  });

  function invalidate() {
    queryClient.invalidateQueries({ queryKey: ["instance-mods", instanceId] });
  }

  const onError = (e: unknown) => notify.error({ title: "Action impossible sur ce mod", message: errorMessage(e) });

  const toggleMutation = useMutation({
    mutationFn: (vars: { fileName: string; enabled: boolean }) =>
      instanceModsApi.setEnabled(instanceId, vars.fileName, vars.enabled),
    onSuccess: invalidate,
    onError,
  });

  const deleteMutation = useMutation({
    mutationFn: (fileName: string) => instanceModsApi.delete(instanceId, fileName),
    onSuccess: (_, fileName) => {
      invalidate();
      setToDelete(null);
      notify.success({ title: "Mod supprimé", message: prettyModName(fileName), history: false });
    },
    onError,
  });

  const addMutation = useMutation({
    mutationFn: (sourcePath: string) => instanceModsApi.add(instanceId, sourcePath),
    onSuccess: () => {
      invalidate();
      notify.success({ title: "Mod ajouté", history: false });
    },
    onError,
  });

  async function addMod() {
    const picked = await open({
      multiple: false,
      filters: [{ name: "Mod (.jar)", extensions: ["jar"] }],
    });
    if (typeof picked === "string") {
      addMutation.mutate(picked);
    }
  }

  const mods = useMemo(() => modsQuery.data ?? [], [modsQuery.data]);
  const enabledCount = mods.filter((m) => m.enabled).length;
  const totalSize = mods.reduce((n, m) => n + m.size, 0);
  const visible = useMemo(() => {
    const needle = search.trim().toLowerCase();
    return mods
      .filter((m) => filter === "all" || (filter === "enabled") === m.enabled)
      .filter((m) => !needle || m.file_name.toLowerCase().includes(needle));
  }, [mods, filter, search]);

  const filters: { value: Filter; label: string; count: number }[] = [
    { value: "all", label: "Tous", count: mods.length },
    { value: "enabled", label: "Actifs", count: enabledCount },
    { value: "disabled", label: "Désactivés", count: mods.length - enabledCount },
  ];

  return (
    <div className="flex flex-1 flex-col">
      <PageHeader
        eyebrow={instance?.name ?? "Instance"}
        title="Mods"
        description={
          mods.length > 0
            ? `${enabledCount} actif${enabledCount > 1 ? "s" : ""} sur ${mods.length} · ${formatBytes(totalSize)}`
            : "Active, désactive, supprime ou ajoute des mods."
        }
        action={
          <>
            <Button variant="ghost" size="sm" className="gap-1.5" onClick={() => navigate(`/instances/${instanceId}`)}>
              <ArrowLeft aria-hidden="true" />
              Retour
            </Button>
            <Button variant="outline" size="sm" className="gap-1.5" onClick={() => instancesApi.openFolder(instanceId)}>
              <FolderOpen aria-hidden="true" />
              Dossier
            </Button>
            <Button size="sm" onClick={addMod} disabled={addMutation.isPending} className="bg-gradient-brand gap-1.5">
              {addMutation.isPending ? (
                <Loader2 className="animate-spin" aria-hidden="true" />
              ) : (
                <Plus aria-hidden="true" />
              )}
              Ajouter un mod…
            </Button>
          </>
        }
      />

      {modsQuery.isLoading ? (
        <div className="space-y-2">
          {[0, 1, 2, 3, 4].map((i) => (
            <Skeleton key={i} className="h-14 rounded-xl" />
          ))}
        </div>
      ) : mods.length === 0 ? (
        <EmptyState
          icon={Blocks}
          title="Aucun mod"
          description="Ajoute un fichier .jar pour l'installer dans cette instance."
          action={
            <Button onClick={addMod} className="gap-1.5">
              <Plus aria-hidden="true" />
              Ajouter un mod
            </Button>
          }
        />
      ) : (
        <>
          <div className="mb-3 flex flex-wrap items-center gap-2">
            <div className="glass flex rounded-lg p-0.5">
              {filters.map((f) => (
                <button
                  key={f.value}
                  onClick={() => setFilter(f.value)}
                  className={cn(
                    "rounded-md px-3 py-1 text-xs font-medium transition-colors",
                    filter === f.value
                      ? "bg-primary text-primary-foreground"
                      : "text-muted-foreground hover:text-foreground",
                  )}
                >
                  {f.label} <span className="tabular-nums opacity-70">{f.count}</span>
                </button>
              ))}
            </div>
            <div className="relative ml-auto">
              <Search
                className="pointer-events-none absolute top-1/2 left-2.5 size-3.5 -translate-y-1/2 text-muted-foreground"
                aria-hidden="true"
              />
              <Input
                type="search"
                placeholder="Rechercher un mod…"
                value={search}
                onChange={(e) => setSearch(e.target.value)}
                className="h-8 w-56 pl-8!"
              />
            </div>
          </div>

          <motion.div
            key={filter}
            initial="hidden"
            animate="show"
            className="glass divide-y divide-border/60 overflow-hidden rounded-2xl"
          >
            {visible.length === 0 && (
              <p className="px-4 py-8 text-center text-sm text-muted-foreground">Aucun mod ne correspond.</p>
            )}
            {visible.map((mod, index) => (
              <motion.div
                key={mod.file_name}
                variants={listItem}
                custom={index}
                className={cn("flex items-center gap-3 px-4 py-2.5 transition-opacity", !mod.enabled && "opacity-55")}
              >
                <div
                  className={cn(
                    "flex size-9 shrink-0 items-center justify-center rounded-lg",
                    mod.enabled ? "bg-primary/12 text-primary" : "bg-muted text-muted-foreground",
                  )}
                >
                  <Puzzle className="size-4" aria-hidden="true" />
                </div>
                <div className="min-w-0 flex-1">
                  <p className="truncate text-sm font-medium capitalize">{prettyModName(mod.file_name)}</p>
                  <p className="truncate text-xs text-muted-foreground">
                    {mod.file_name} · {formatBytes(mod.size)}
                  </p>
                </div>
                <Switch
                  checked={mod.enabled}
                  aria-label={mod.enabled ? "Désactiver" : "Activer"}
                  onCheckedChange={(enabled) => toggleMutation.mutate({ fileName: mod.file_name, enabled })}
                />
                <Button variant="ghost" size="icon-sm" title="Supprimer" onClick={() => setToDelete(mod)}>
                  <Trash2 aria-hidden="true" />
                </Button>
              </motion.div>
            ))}
          </motion.div>
        </>
      )}

      <ConfirmDialog
        open={toDelete !== null}
        onOpenChange={(o) => !o && setToDelete(null)}
        title="Supprimer ce mod ?"
        description={toDelete ? `« ${toDelete.file_name} » sera supprimé du dossier mods.` : ""}
        confirmLabel="Supprimer"
        destructive
        pending={deleteMutation.isPending}
        onConfirm={() => toDelete && deleteMutation.mutate(toDelete.file_name)}
      />
    </div>
  );
}
