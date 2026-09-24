import { useMemo, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useNavigate } from "react-router";
import { ArrowRight, Loader2, MemoryStick, PackageSearch, Sparkles } from "lucide-react";

import { LoaderBadge } from "@/components/instance/LoaderBadge";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { useSystemMemory } from "@/hooks/useInstanceInfo";
import { formatGb } from "@/lib/format";
import { notifyInstallResult } from "@/lib/installResult";
import { notify } from "@/lib/notify";
import { adviseRam } from "@/lib/ramAdvice";
import {
  errorMessage,
  instancesApi,
  isCancelled,
  providersApi,
  type ModpackSummary,
  type ProviderId,
} from "@/services/tauri";

import { ModpackChangelog } from "./ModpackChangelog";

interface ModpackDetailDialogProps {
  provider: ProviderId;
  pack: ModpackSummary | null;
  onOpenChange: (open: boolean) => void;
  /** When set, the dialog installs the chosen version into this existing
   * instance instead of creating a new one — used for the "update available"
   * flow on an already-installed modpack. */
  updateInstanceId?: string;
  /** Opens another provider's copy of the pack instead — offered when a
   * CurseForge pack is also published by FTB. */
  onSwitchPack?: (provider: ProviderId, pack: ModpackSummary) => void;
}

interface InstallVars {
  packId: string;
  versionId: string;
  packName: string;
  packIconUrl: string | null;
  instanceName: string;
}

export function ModpackDetailDialog({
  provider,
  pack,
  onOpenChange,
  updateInstanceId,
  onSwitchPack,
}: ModpackDetailDialogProps) {
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const [pickedVersionId, setVersionId] = useState("");
  const [instanceName, setInstanceName] = useState("");
  const isUpdate = !!updateInstanceId;
  const { data: memory } = useSystemMemory();

  const versionsQuery = useQuery({
    queryKey: ["modpack-versions", provider, pack?.id],
    queryFn: () => providersApi.getVersions(provider, pack!.id),
    enabled: pack !== null,
  });
  // FTB mirrors its own packs' files, so its copy installs without the
  // manual downloads some CurseForge authors impose.
  const ftbQuery = useQuery({
    queryKey: ["ftb-equivalent", pack?.id],
    queryFn: () => providersApi.ftbEquivalent(Number(pack!.id), pack!.name),
    enabled: pack !== null && provider === "curseforge" && !isUpdate && !!onSwitchPack,
    staleTime: Infinity,
  });
  const ftbPack = ftbQuery.data;
  // Newest version preselected: it's what most people want to install.
  const versionId = pickedVersionId || versionsQuery.data?.[0]?.id || "";

  // Variables are captured explicitly (not read from `pack`/`versionId` at
  // success time) since the dialog closes and navigates away immediately on
  // submit — by the time this resolves, `pack` may already be null.
  const installMutation = useMutation({
    mutationFn: (vars: InstallVars) =>
      instancesApi.installModpack(
        provider,
        vars.packId,
        vars.versionId,
        vars.packName,
        vars.packIconUrl,
        vars.instanceName,
      ),
    onSuccess: (result, vars) => {
      queryClient.invalidateQueries({ queryKey: ["instances"] });
      queryClient.invalidateQueries({ queryKey: ["instance-mods", result.instance.id] });
      notifyInstallResult(result, `${vars.packName} installé`, () => navigate(`/instances/${result.instance.id}`));
    },
    onError: (e, vars) =>
      isCancelled(e)
        ? notify.info({ title: `Installation de ${vars.packName} annulée`, history: false })
        : notify.error({ title: `Échec de l'installation de ${vars.packName}`, message: errorMessage(e) }),
  });

  const updateMutation = useMutation({
    mutationFn: (vars: { versionId: string }) => instancesApi.updateModpack(updateInstanceId!, vars.versionId),
    onSuccess: (result) => {
      queryClient.invalidateQueries({ queryKey: ["instances"] });
      queryClient.invalidateQueries({ queryKey: ["modpack-versions", provider, pack?.id] });
      queryClient.invalidateQueries({ queryKey: ["instance-mods", result.instance.id] });
      queryClient.invalidateQueries({ queryKey: ["instance", result.instance.id] });
      notifyInstallResult(result, `${result.instance.name} mis à jour`, () =>
        navigate(`/instances/${result.instance.id}`),
      );
    },
    onError: (e) =>
      isCancelled(e)
        ? notify.info({ title: "Mise à jour annulée", history: false })
        : notify.error({ title: "Échec de la mise à jour", message: errorMessage(e) }),
  });

  const activeMutation = isUpdate ? updateMutation : installMutation;
  const selected = versionsQuery.data?.find((v) => v.id === versionId);
  const advice = useMemo(
    () =>
      selected
        ? adviseRam({
            loader: selected.loader,
            modCount: null,
            minecraftVersion: selected.minecraft_version,
            isModpack: true,
            systemTotalMb: memory?.total_mb ?? null,
          })
        : null,
    [selected, memory?.total_mb],
  );

  function close() {
    onOpenChange(false);
    setVersionId("");
    setInstanceName("");
  }

  function submitInstall() {
    if (!pack) return;
    if (isUpdate) {
      updateMutation.mutate({ versionId });
    } else {
      installMutation.mutate({
        packId: pack.id,
        versionId,
        packName: pack.name,
        packIconUrl: pack.icon_url,
        instanceName: instanceName.trim() || pack.name,
      });
    }
    close();
    if (!isUpdate) navigate("/");
  }

  return (
    <Dialog open={pack !== null} onOpenChange={(next) => (next ? undefined : close())}>
      <DialogContent className="overflow-hidden sm:max-w-lg">
        <div className="relative -mx-4 -mt-4 mb-1 h-28 overflow-hidden" aria-hidden="true">
          {pack?.icon_url && (
            <img
              src={pack.icon_url}
              alt=""
              className="absolute inset-0 size-full scale-125 object-cover opacity-50 blur-xl"
            />
          )}
          <div className="bg-gradient-brand absolute inset-0 opacity-40 mix-blend-overlay" />
          <div className="absolute inset-0 bg-gradient-to-b from-transparent to-background" />
        </div>
        <DialogHeader className="-mt-16 flex-row items-end gap-4 space-y-0">
          {pack?.icon_url ? (
            <img src={pack.icon_url} alt="" className="relative size-20 rounded-2xl shadow-xl ring-4 ring-background" />
          ) : (
            <div className="bg-gradient-brand relative flex size-20 items-center justify-center rounded-2xl ring-4 ring-background">
              <PackageSearch className="size-8 text-primary-foreground" aria-hidden="true" />
            </div>
          )}
          <div className="relative min-w-0 flex-1 pb-1 text-left">
            <DialogTitle className="truncate text-xl">{pack?.name}</DialogTitle>
            {pack?.author && <p className="text-xs text-muted-foreground">par {pack.author}</p>}
          </div>
        </DialogHeader>
        <DialogDescription className="line-clamp-3">
          {isUpdate
            ? "Choisis la version cible. Tes mondes sont sauvegardés avant, et tes options de jeu conservées."
            : pack?.summary}
        </DialogDescription>

        <div className="space-y-4">
          {ftbPack && onSwitchPack && (
            <div className="flex items-center gap-3 rounded-xl border border-[#e5484d]/40 bg-[#e5484d]/10 p-3">
              <Sparkles className="size-4 shrink-0 text-[#e5484d]" aria-hidden="true" />
              <p className="flex-1 text-xs">
                Ce pack est publié par FTB : l'installer depuis FTB évite les fichiers à télécharger à la main.
              </p>
              <Button
                size="sm"
                variant="outline"
                className="shrink-0 gap-1.5"
                onClick={() => {
                  setVersionId("");
                  onSwitchPack("ftb", ftbPack);
                }}
              >
                Voir sur FTB
                <ArrowRight className="size-3.5" aria-hidden="true" />
              </Button>
            </div>
          )}

          {!isUpdate && (
            <div className="space-y-1.5">
              <Label htmlFor="modpack-instance-name">Nom de l'instance</Label>
              <Input
                id="modpack-instance-name"
                placeholder={pack?.name}
                value={instanceName}
                onChange={(e) => setInstanceName(e.target.value)}
              />
            </div>
          )}

          <div className="space-y-1.5">
            <Label>Version</Label>
            {versionsQuery.isLoading ? (
              <p className="flex items-center gap-2 text-sm text-muted-foreground">
                <Loader2 className="size-3.5 animate-spin" aria-hidden="true" /> Chargement…
              </p>
            ) : (
              <Select value={versionId} onValueChange={setVersionId}>
                <SelectTrigger className="w-full">
                  <SelectValue placeholder="Choisir une version" />
                </SelectTrigger>
                <SelectContent>
                  {(versionsQuery.data ?? []).map((v) => (
                    <SelectItem key={v.id} value={v.id}>
                      {v.name} ({v.minecraft_version}
                      {v.loader !== "vanilla" ? ` · ${v.loader}` : ""})
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            )}
            {selected && (
              <div className="flex flex-wrap items-center gap-2 pt-1 text-xs text-muted-foreground">
                <LoaderBadge loader={selected.loader} version={selected.loader_version} />
                <span>Minecraft {selected.minecraft_version}</span>
                {advice && (
                  <span className="flex items-center gap-1">
                    <MemoryStick className="size-3.5" aria-hidden="true" />
                    RAM conseillée : {formatGb(advice.recommendedMb)}
                  </span>
                )}
              </div>
            )}
          </div>

          {pack && versionId && <ModpackChangelog provider={provider} packId={pack.id} versionId={versionId} />}
        </div>

        <DialogFooter>
          <Button variant="outline" onClick={close}>
            Annuler
          </Button>
          <Button
            onClick={submitInstall}
            disabled={!versionId || activeMutation.isPending}
            className="bg-gradient-brand shadow-glow gap-1.5"
          >
            {activeMutation.isPending && <Loader2 className="size-3.5 animate-spin" aria-hidden="true" />}
            {isUpdate ? "Mettre à jour" : "Installer"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
