import { useCallback, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { convertFileSrc } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { motion } from "motion/react";
import { Archive, FolderOpen, Globe, History, Import, Loader2, MoreHorizontal, Skull, Trash2 } from "lucide-react";

import { EmptyState } from "@/components/EmptyState";
import { MinecraftGrassIcon } from "@/components/MinecraftGrassIcon";
import { Skeleton } from "@/components/Skeleton";
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
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { useFileDrop } from "@/hooks/useFileDrop";
import { formatBytes, formatRelative } from "@/lib/format";
import { listItem } from "@/lib/motion";
import { notify } from "@/lib/notify";
import { worldsApi, type World } from "@/services/content";
import { errorMessage, instancesApi, type Instance } from "@/services/tauri";

import { WorldBackupsDialog } from "./WorldBackupsDialog";

const MODES: Record<string, { label: string; className: string }> = {
  survival: { label: "Survie", className: "bg-emerald-500/15 text-emerald-600 dark:text-emerald-400" },
  creative: { label: "Créatif", className: "bg-sky-500/15 text-sky-600 dark:text-sky-400" },
  adventure: { label: "Aventure", className: "bg-amber-500/15 text-amber-600 dark:text-amber-400" },
  spectator: { label: "Spectateur", className: "bg-violet-500/15 text-violet-600 dark:text-violet-400" },
};

function WorldCard({
  world,
  index,
  onBackup,
  onOpen,
  onDelete,
  backingUp,
}: {
  world: World;
  index: number;
  onBackup: () => void;
  onOpen: () => void;
  onDelete: () => void;
  backingUp: boolean;
}) {
  const mode = world.game_mode ? MODES[world.game_mode] : null;
  return (
    <motion.div
      variants={listItem}
      custom={index}
      initial="hidden"
      animate="show"
      className="glass group flex items-center gap-3 rounded-2xl p-3"
    >
      {world.icon_path ? (
        <img
          src={convertFileSrc(world.icon_path)}
          alt=""
          className="size-16 shrink-0 rounded-xl object-cover [image-rendering:pixelated]"
        />
      ) : (
        <MinecraftGrassIcon className="size-16 shrink-0 rounded-xl" />
      )}
      <div className="min-w-0 flex-1 space-y-1">
        <p className="truncate font-semibold" title={world.folder}>
          {world.name}
        </p>
        <div className="flex flex-wrap items-center gap-1.5 text-[0.7rem]">
          {world.hardcore ? (
            <span className="inline-flex items-center gap-1 rounded-full bg-red-500/15 px-2 py-px font-semibold text-red-600 dark:text-red-400">
              <Skull className="size-3" aria-hidden="true" />
              Hardcore
            </span>
          ) : (
            mode && <span className={`rounded-full px-2 py-px font-semibold ${mode.className}`}>{mode.label}</span>
          )}
          {world.cheats && <span className="rounded-full bg-muted px-2 py-px text-muted-foreground">Commandes</span>}
          {world.version && (
            <span className="rounded-full bg-muted px-2 py-px text-muted-foreground">{world.version}</span>
          )}
        </div>
        <p className="truncate text-xs text-muted-foreground">
          {world.last_played ? `Joué ${formatRelative(world.last_played)}` : "Jamais joué"} · {formatBytes(world.size)}
        </p>
      </div>
      <DropdownMenu>
        <DropdownMenuTrigger asChild>
          <Button variant="ghost" size="icon-sm" aria-label={`Actions pour ${world.name}`}>
            {backingUp ? (
              <Loader2 className="animate-spin" aria-hidden="true" />
            ) : (
              <MoreHorizontal aria-hidden="true" />
            )}
          </Button>
        </DropdownMenuTrigger>
        <DropdownMenuContent align="end" className="w-52">
          <DropdownMenuItem className="gap-2" onClick={onBackup} disabled={backingUp}>
            <Archive className="size-3.5" aria-hidden="true" />
            Sauvegarder
          </DropdownMenuItem>
          <DropdownMenuItem className="gap-2" onClick={onOpen}>
            <FolderOpen className="size-3.5" aria-hidden="true" />
            Ouvrir le dossier
          </DropdownMenuItem>
          <DropdownMenuSeparator />
          <DropdownMenuItem className="gap-2 text-destructive focus:text-destructive" onClick={onDelete}>
            <Trash2 className="size-3.5" aria-hidden="true" />
            Supprimer
          </DropdownMenuItem>
        </DropdownMenuContent>
      </DropdownMenu>
    </motion.div>
  );
}

/** The instance's singleplayer worlds: backups, restore, import, delete. */
export function WorldsTab({ instance }: { instance: Instance }) {
  const queryClient = useQueryClient();
  const queryKey = ["worlds", instance.id];
  const worlds = useQuery({ queryKey, queryFn: () => worldsApi.list(instance.id) });
  const [backupsOpen, setBackupsOpen] = useState(false);
  const [toDelete, setToDelete] = useState<World | null>(null);
  const [backupFirst, setBackupFirst] = useState(true);

  const refresh = useCallback(() => {
    queryClient.invalidateQueries({ queryKey: ["worlds", instance.id] });
    queryClient.invalidateQueries({ queryKey: ["world-backups", instance.id] });
    queryClient.invalidateQueries({ queryKey: ["content-summary", instance.id] });
  }, [queryClient, instance.id]);

  const backup = useMutation({
    mutationFn: (world: World) => worldsApi.backup(instance.id, world.folder),
    onSuccess: (_, world) => {
      refresh();
      notify.success({
        title: `« ${world.name} » sauvegardé`,
        message: "Retrouve tes sauvegardes dans « Sauvegardes ».",
        history: false,
      });
    },
    onError: (e) => notify.error({ title: "Sauvegarde impossible", message: errorMessage(e) }),
  });

  const remove = useMutation({
    mutationFn: (vars: { world: World; backupFirst: boolean }) =>
      worldsApi.delete(instance.id, vars.world.folder, vars.backupFirst),
    onSuccess: (_, vars) => {
      refresh();
      setToDelete(null);
      notify.success({
        title: `« ${vars.world.name} » supprimé`,
        message: vars.backupFirst ? "Une sauvegarde a été gardée : tu peux le restaurer." : undefined,
        history: false,
      });
    },
    onError: (e) => notify.error({ title: "Suppression impossible", message: errorMessage(e) }),
  });

  const importWorlds = useMutation({
    mutationFn: (paths: string[]) => worldsApi.import(instance.id, paths),
    onSuccess: (created) => {
      refresh();
      notify.success({
        title: created.length > 1 ? `${created.length} mondes importés` : "Monde importé",
        message: created.join(", "),
        history: false,
      });
    },
    onError: (e) => notify.error({ title: "Import impossible", message: errorMessage(e) }),
  });

  async function pickZip() {
    const picked = await open({ multiple: true, filters: [{ name: "Monde Minecraft (.zip)", extensions: ["zip"] }] });
    const paths = Array.isArray(picked) ? picked : typeof picked === "string" ? [picked] : [];
    if (paths.length > 0) importWorlds.mutate(paths);
  }

  const dragging = useFileDrop(useCallback((paths: string[]) => importWorlds.mutate(paths), [importWorlds]));

  const list = worlds.data ?? [];

  return (
    <>
      <div className="mb-3 flex flex-wrap items-center gap-2">
        <p className="text-sm text-muted-foreground">
          {list.length > 0 && `${list.length} monde${list.length > 1 ? "s" : ""}`}
        </p>
        <div className="ml-auto flex gap-2">
          <Button variant="outline" size="sm" className="gap-1.5" onClick={() => setBackupsOpen(true)}>
            <History aria-hidden="true" />
            Sauvegardes
          </Button>
          <Button
            variant="outline"
            size="sm"
            className="gap-1.5"
            onClick={() => instancesApi.openFolder(instance.id, "saves")}
          >
            <FolderOpen aria-hidden="true" />
            Dossier
          </Button>
          <Button size="sm" className="gap-1.5" onClick={pickZip} disabled={importWorlds.isPending}>
            {importWorlds.isPending ? (
              <Loader2 className="animate-spin" aria-hidden="true" />
            ) : (
              <Import aria-hidden="true" />
            )}
            Importer un monde
          </Button>
        </div>
      </div>

      {worlds.isLoading ? (
        <div className="grid gap-3 lg:grid-cols-2">
          {[0, 1, 2, 3].map((i) => (
            <Skeleton key={i} className="h-24 rounded-2xl" />
          ))}
        </div>
      ) : list.length === 0 ? (
        <EmptyState
          icon={Globe}
          title="Aucun monde"
          description="Crée un monde en jeu, ou importe une map (.zip) : glisse-la sur la fenêtre."
          action={
            <Button onClick={pickZip} className="gap-1.5">
              <Import aria-hidden="true" />
              Importer un monde
            </Button>
          }
        />
      ) : (
        <div className="grid gap-3 lg:grid-cols-2">
          {list.map((world, index) => (
            <WorldCard
              key={world.folder}
              world={world}
              index={index}
              backingUp={backup.isPending && backup.variables?.folder === world.folder}
              onBackup={() => backup.mutate(world)}
              onOpen={() => worldsApi.openFolder(instance.id, world.folder)}
              onDelete={() => {
                setBackupFirst(true);
                setToDelete(world);
              }}
            />
          ))}
        </div>
      )}

      {dragging && (
        <div className="pointer-events-none fixed inset-4 z-50 flex items-center justify-center rounded-3xl border-2 border-dashed border-primary bg-background/80 backdrop-blur-sm">
          <p className="text-lg font-semibold text-primary">Dépose une map (.zip ou dossier) pour l'importer</p>
        </div>
      )}

      <WorldBackupsDialog instance={instance} open={backupsOpen} onOpenChange={setBackupsOpen} onRestored={refresh} />

      <Dialog open={toDelete !== null} onOpenChange={(o) => !o && setToDelete(null)}>
        <DialogContent className="sm:max-w-md">
          <DialogHeader>
            <DialogTitle>Supprimer « {toDelete?.name} » ?</DialogTitle>
            <DialogDescription>Le monde disparaîtra de ta liste de mondes en jeu.</DialogDescription>
          </DialogHeader>
          <label className="flex cursor-pointer items-start gap-2.5 rounded-lg bg-muted/60 p-3 text-sm">
            <Checkbox checked={backupFirst} onCheckedChange={(v) => setBackupFirst(v === true)} className="mt-0.5" />
            <span>
              <span className="font-medium">Garder une sauvegarde</span>
              <span className="block text-xs text-muted-foreground">
                Recommandé : tu pourras le restaurer depuis « Sauvegardes ».
              </span>
            </span>
          </label>
          <DialogFooter>
            <Button variant="outline" onClick={() => setToDelete(null)}>
              Annuler
            </Button>
            <Button
              variant="destructive"
              disabled={remove.isPending}
              className="gap-1.5"
              onClick={() => toDelete && remove.mutate({ world: toDelete, backupFirst })}
            >
              {remove.isPending && <Loader2 className="size-3.5 animate-spin" aria-hidden="true" />}
              Supprimer
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </>
  );
}
