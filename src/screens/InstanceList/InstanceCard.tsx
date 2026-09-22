import { useState, type KeyboardEvent } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useNavigate } from "react-router";
import { motion } from "motion/react";
import { FolderOpen, MoreHorizontal, Puzzle, RefreshCw, Settings2, Timer, Trash2 } from "lucide-react";

import { ConfirmDialog } from "@/components/ConfirmDialog";
import { InstanceIcon } from "@/components/instance/InstanceIcon";
import { LOADER_META, LoaderBadge } from "@/components/instance/LoaderBadge";
import { PlayButton } from "@/components/instance/PlayButton";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { useLaunchInstance } from "@/hooks/useLaunchInstance";
import { formatDuration, formatRelative } from "@/lib/format";
import { fadeUp } from "@/lib/motion";
import { notify } from "@/lib/notify";
import { cn } from "@/lib/utils";
import { errorMessage, instancesApi, providersApi, type Instance, type ProviderId } from "@/services/tauri";
import type { CardDensity } from "@/store/preferencesStore";

import { ModpackDetailDialog } from "../ModpackBrowser/ModpackDetailDialog";

function useModpackUpdate(instance: Instance) {
  const modpack = instance.modpack;
  const { data } = useQuery({
    queryKey: ["modpack-versions", modpack?.provider, modpack?.pack_id],
    queryFn: () => providersApi.getVersions(modpack!.provider as ProviderId, modpack!.pack_id),
    enabled: !!modpack,
    staleTime: 5 * 60 * 1000,
  });
  const latest = data?.[0];
  return !!modpack && !!latest && latest.id !== modpack.version_id ? latest : null;
}

function StatusChips({ running, updateAvailable }: { running: boolean; updateAvailable: boolean }) {
  return (
    <div className="flex gap-1.5">
      {running && (
        <span className="flex items-center gap-1 rounded-full bg-success/90 px-2 py-0.5 text-[0.65rem] font-bold text-white shadow">
          <span className="size-1.5 animate-pulse rounded-full bg-white" aria-hidden="true" />
          EN JEU
        </span>
      )}
      {updateAvailable && (
        <span className="flex items-center gap-1 rounded-full bg-info/90 px-2 py-0.5 text-[0.65rem] font-bold text-white shadow">
          <RefreshCw className="size-2.5" aria-hidden="true" />
          MAJ
        </span>
      )}
    </div>
  );
}

export function InstanceCard({ instance, density = "grid" }: { instance: Instance; density?: CardDensity }) {
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const { running, installing, installPercent } = useLaunchInstance(instance);
  const update = useModpackUpdate(instance);
  const [updating, setUpdating] = useState(false);
  const [confirmDelete, setConfirmDelete] = useState(false);
  const color = LOADER_META[instance.loader].color;

  const deleteMutation = useMutation({
    mutationFn: () => instancesApi.delete(instance.id),
    onSuccess: () => {
      setConfirmDelete(false);
      queryClient.invalidateQueries({ queryKey: ["instances"] });
      notify.success({ title: "Instance supprimée", message: instance.name });
    },
    onError: (e) => notify.error({ title: "Suppression impossible", message: errorMessage(e) }),
  });

  const openDetail = () => navigate(running ? `/instances/${instance.id}/launch` : `/instances/${instance.id}`);
  const meta =
    instance.play_time_seconds > 0
      ? formatDuration(instance.play_time_seconds)
      : instance.last_played_at
        ? formatRelative(instance.last_played_at)
        : "Jamais jouée";

  const menu = (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <Button variant="ghost" size="icon-sm" title="Plus d'actions" onClick={(e) => e.stopPropagation()}>
          <MoreHorizontal aria-hidden="true" />
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end" onClick={(e) => e.stopPropagation()}>
        <DropdownMenuItem className="gap-2" onClick={() => navigate(`/instances/${instance.id}`)}>
          <Settings2 className="size-4" aria-hidden="true" />
          Réglages
        </DropdownMenuItem>
        {instance.loader !== "vanilla" && (
          <DropdownMenuItem className="gap-2" onClick={() => navigate(`/instances/${instance.id}/mods`)}>
            <Puzzle className="size-4" aria-hidden="true" />
            Gérer les mods
          </DropdownMenuItem>
        )}
        <DropdownMenuItem className="gap-2" onClick={() => instancesApi.openFolder(instance.id)}>
          <FolderOpen className="size-4" aria-hidden="true" />
          Ouvrir le dossier
        </DropdownMenuItem>
        {update && (
          <DropdownMenuItem className="gap-2 text-info" disabled={running} onClick={() => setUpdating(true)}>
            <RefreshCw className="size-4" aria-hidden="true" />
            Mettre à jour ({update.name})
          </DropdownMenuItem>
        )}
        <DropdownMenuSeparator />
        <DropdownMenuItem className="gap-2 text-destructive" disabled={running} onClick={() => setConfirmDelete(true)}>
          <Trash2 className="size-4" aria-hidden="true" />
          Supprimer
        </DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
  );

  const clickable = {
    role: installing ? undefined : "button",
    tabIndex: installing ? undefined : 0,
    onClick: installing ? undefined : openDetail,
    onKeyDown: installing
      ? undefined
      : (e: KeyboardEvent) => (e.key === "Enter" || e.key === " ") && (e.preventDefault(), openDetail()),
  };

  return (
    <motion.div variants={fadeUp} layout whileHover={installing ? undefined : { y: -4 }} className="group">
      {density === "grid" ? (
        <div
          {...clickable}
          className={cn(
            "glass relative flex h-full cursor-pointer flex-col overflow-hidden rounded-2xl transition-shadow hover:shadow-xl",
            installing && "cursor-default",
          )}
        >
          <div className="relative h-24 overflow-hidden">
            <div
              className="absolute inset-0"
              style={{
                backgroundImage: `linear-gradient(135deg, color-mix(in oklab, ${color} 55%, transparent), color-mix(in oklab, ${color} 10%, transparent))`,
              }}
            />
            {instance.icon_url && (
              <img
                src={instance.icon_url}
                alt=""
                aria-hidden="true"
                className="absolute inset-0 size-full scale-125 object-cover opacity-40 blur-md transition-transform duration-500 group-hover:scale-150"
              />
            )}
            <div className="absolute top-2.5 right-2.5">
              <StatusChips running={running} updateAvailable={!!update} />
            </div>
            {installing && (
              <div className="absolute inset-x-0 bottom-0 h-1.5 bg-black/30">
                <div className="bg-gradient-brand h-full transition-[width]" style={{ width: `${installPercent}%` }} />
                <div className="shimmer-bg absolute inset-0 animate-shimmer" />
              </div>
            )}
          </div>

          <div className="relative -mt-9 flex flex-1 flex-col gap-3 px-4 pb-4">
            <InstanceIcon
              instance={instance}
              className="size-16 rounded-xl shadow-lg ring-4 ring-card transition-transform duration-300 group-hover:scale-105 group-hover:-rotate-3"
            />
            <div className="min-w-0 space-y-1.5">
              <h3 className="truncate text-base font-bold">{instance.name}</h3>
              <div className="flex flex-wrap items-center gap-1.5">
                <LoaderBadge loader={instance.loader} />
                <span className="text-xs text-muted-foreground">{instance.minecraft_version}</span>
              </div>
            </div>
            <div className="mt-auto flex items-center justify-between gap-2 pt-1">
              <span className="flex min-w-0 items-center gap-1 truncate text-xs text-muted-foreground">
                <Timer className="size-3.5 shrink-0" aria-hidden="true" />
                {installing ? `Installation… ${installPercent}%` : meta}
              </span>
              <div className="flex items-center gap-1">
                {menu}
                <PlayButton instance={instance} />
              </div>
            </div>
          </div>
        </div>
      ) : (
        <div
          {...clickable}
          className="glass relative flex cursor-pointer items-center gap-3 overflow-hidden rounded-xl p-2.5 pr-3 transition-shadow hover:shadow-lg"
        >
          <div className="absolute inset-y-0 left-0 w-1" style={{ backgroundColor: color }} aria-hidden="true" />
          <InstanceIcon instance={instance} className="ml-1 size-11 rounded-lg" />
          <div className="min-w-0 flex-1">
            <div className="flex items-center gap-2">
              <h3 className="truncate text-sm font-bold">{instance.name}</h3>
              <StatusChips running={running} updateAvailable={!!update} />
            </div>
            <div className="mt-0.5 flex items-center gap-2 text-xs text-muted-foreground">
              <LoaderBadge loader={instance.loader} className="h-4 px-1.5 text-[0.62rem]" />
              <span>{instance.minecraft_version}</span>
              {density === "list" && (
                <span className="truncate">· {installing ? `Installation… ${installPercent}%` : meta}</span>
              )}
            </div>
          </div>
          {menu}
          <PlayButton instance={instance} />
        </div>
      )}

      {instance.modpack && (
        <ModpackDetailDialog
          provider={instance.modpack.provider as ProviderId}
          pack={
            updating
              ? {
                  id: instance.modpack.pack_id,
                  provider: instance.modpack.provider,
                  name: instance.modpack.pack_name,
                  author: "",
                  icon_url: instance.icon_url,
                  summary: "",
                }
              : null
          }
          onOpenChange={(open) => !open && setUpdating(false)}
          updateInstanceId={instance.id}
        />
      )}
      <ConfirmDialog
        open={confirmDelete}
        onOpenChange={setConfirmDelete}
        title={`Supprimer « ${instance.name} » ?`}
        description="Le dossier de l'instance sera effacé, y compris ses mondes, captures d'écran et configurations. Cette action est définitive."
        confirmLabel="Supprimer définitivement"
        destructive
        pending={deleteMutation.isPending}
        onConfirm={() => deleteMutation.mutate()}
      />
    </motion.div>
  );
}
