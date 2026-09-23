import { useCallback, useMemo, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useNavigate, useParams } from "react-router";
import { open } from "@tauri-apps/plugin-dialog";
import { motion } from "motion/react";
import { ArrowLeft, Blocks, Compass, FolderOpen, Loader2, Plus, Puzzle, RefreshCw, Search, Trash2 } from "lucide-react";

import { ConfirmDialog } from "@/components/ConfirmDialog";
import { EmptyState } from "@/components/EmptyState";
import { PageHeader } from "@/components/PageHeader";
import { Skeleton } from "@/components/Skeleton";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Switch } from "@/components/ui/switch";
import { useFileDrop } from "@/hooks/useFileDrop";
import { formatBytes } from "@/lib/format";
import { listItem } from "@/lib/motion";
import { notify } from "@/lib/notify";
import { cn } from "@/lib/utils";
import { errorMessage, instanceModsApi, instancesApi, type ModEntry } from "@/services/tauri";

import { ContentBrowserDialog } from "./ContentBrowserDialog";
import { ModUpdatesDialog } from "./ModUpdatesDialog";

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
  const [browseOpen, setBrowseOpen] = useState(false);
  const [updatesOpen, setUpdatesOpen] = useState(false);

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
    mutationFn: (sourcePaths: string[]) => instanceModsApi.add(instanceId, sourcePaths),
    onSuccess: (_, paths) => {
      invalidate();
      notify.success({ title: paths.length > 1 ? `${paths.length} mods ajoutés` : "Mod ajouté", history: false });
    },
    onError,
  });

  async function addMod() {
    const picked = await open({
      multiple: true,
      filters: [{ name: "Mod (.jar)", extensions: ["jar"] }],
    });
    const paths = Array.isArray(picked) ? picked : typeof picked === "string" ? [picked] : [];
    if (paths.length > 0) addMutation.mutate(paths);
  }

  const vanilla = instance?.loader === "vanilla";
  const onDrop = useCallback(
    (paths: string[]) => {
      const jars = paths.filter((p) => p.toLowerCase().endsWith(".jar"));
      if (jars.length > 0) addMutation.mutate(jars);
      else notify.warning({ title: "Seuls les fichiers .jar peuvent être ajoutés ici", history: false });
    },
    [addMutation],
  );
  const dragging = useFileDrop(onDrop, !vanilla);

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
        title={vanilla ? "Contenu" : "Mods"}
        description={
          vanilla
            ? "Ajoute des resource packs et des shaders depuis Modrinth."
            : mods.length > 0
              ? `${enabledCount} actif${enabledCount > 1 ? "s" : ""} sur ${mods.length} · ${formatBytes(totalSize)}`
              : "Active, désactive, supprime ou ajoute des mods."
        }
        action={
          <>
            <Button variant="ghost" size="sm" className="gap-1.5" onClick={() => navigate(`/instances/${instanceId}`)}>
              <ArrowLeft aria-hidden="true" />
              Retour
            </Button>
            <Button
              variant="outline"
              size="sm"
              className="gap-1.5"
              onClick={() => instancesApi.openFolder(instanceId, vanilla ? undefined : "mods")}
            >
              <FolderOpen aria-hidden="true" />
              Dossier
            </Button>
            {!vanilla && (
              <>
                <Button variant="outline" size="sm" className="gap-1.5" onClick={() => setUpdatesOpen(true)}>
                  <RefreshCw aria-hidden="true" />
                  Mises à jour
                </Button>
                <Button
                  variant="outline"
                  size="sm"
                  onClick={addMod}
                  disabled={addMutation.isPending}
                  className="gap-1.5"
                >
                  {addMutation.isPending ? (
                    <Loader2 className="animate-spin" aria-hidden="true" />
                  ) : (
                    <Plus aria-hidden="true" />
                  )}
                  Fichier .jar…
                </Button>
              </>
            )}
            <Button size="sm" onClick={() => setBrowseOpen(true)} className="bg-gradient-brand gap-1.5">
              <Compass aria-hidden="true" />
              Parcourir Modrinth
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
          title={vanilla ? "Instance vanilla" : "Aucun mod"}
          description={
            vanilla
              ? "Cette instance n'a pas de mod loader : ajoute des resource packs ou des shaders."
              : "Parcours Modrinth, ou glisse des fichiers .jar sur la fenêtre."
          }
          action={
            <Button onClick={() => setBrowseOpen(true)} className="gap-1.5">
              <Compass aria-hidden="true" />
              Parcourir Modrinth
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

      {dragging && (
        <div className="pointer-events-none fixed inset-4 z-50 flex items-center justify-center rounded-3xl border-2 border-dashed border-primary bg-background/80 backdrop-blur-sm">
          <p className="text-lg font-semibold text-primary">Dépose tes fichiers .jar pour les ajouter</p>
        </div>
      )}

      {instance && <ContentBrowserDialog instance={instance} open={browseOpen} onOpenChange={setBrowseOpen} />}
      <ModUpdatesDialog instanceId={instanceId} open={updatesOpen} onOpenChange={setUpdatesOpen} />

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
