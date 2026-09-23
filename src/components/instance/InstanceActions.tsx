import { useState } from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { useNavigate } from "react-router";
import { save } from "@tauri-apps/plugin-dialog";
import { Archive, Copy, FolderArchive, Loader2, PackageOpen, Wrench } from "lucide-react";

import { SettingRow, SettingSection } from "@/components/settings/SettingsKit";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Switch } from "@/components/ui/switch";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { notify } from "@/lib/notify";
import { errorMessage, instancesApi, isCancelled, launchApi, type Instance } from "@/services/tauri";
import { runtimeOf, useAppStore } from "@/store/appStore";

/** Maintenance actions for one instance: duplicate, export, repair, back up worlds. */
export function InstanceActions({ instance }: { instance: Instance }) {
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const busy = useAppStore((s) => runtimeOf(s.runtime, instance.id).running);
  const [duplicateOpen, setDuplicateOpen] = useState(false);
  const [copyName, setCopyName] = useState(`${instance.name} (copie)`.slice(0, 64));
  const [includeSaves, setIncludeSaves] = useState(false);

  const duplicate = useMutation({
    mutationFn: () => instancesApi.duplicate(instance.id, copyName),
    onSuccess: (copy) => {
      setDuplicateOpen(false);
      queryClient.invalidateQueries({ queryKey: ["instances"] });
      notify.success({ title: "Instance dupliquée", message: copy.name });
      navigate(`/instances/${copy.id}`);
    },
    onError: (e) => notify.error({ title: "Duplication impossible", message: errorMessage(e) }),
  });

  const exportPack = useMutation({
    mutationFn: async () => {
      const dest = await save({
        title: "Exporter l'instance",
        defaultPath: `${instance.name.replace(/[\\/:*?"<>|]/g, "_")}.mrpack`,
        filters: [{ name: "Modpack Modrinth", extensions: ["mrpack"] }],
      });
      return dest ? instancesApi.export(instance.id, dest, includeSaves) : null;
    },
    onSuccess: (summary) => {
      if (!summary) return;
      notify.success({
        title: "Instance exportée",
        message: `${summary.referenced} fichier(s) référencé(s) sur Modrinth, ${summary.bundled} inclus dans le pack.`,
      });
    },
    onError: (e) => notify.error({ title: "Export impossible", message: errorMessage(e) }),
  });

  const repair = useMutation({
    mutationFn: () => launchApi.repair(instance.id),
    onSuccess: () => notify.success({ title: "Instance réparée", message: "Tous les fichiers ont été vérifiés." }),
    onError: (e) => {
      if (!isCancelled(e)) notify.error({ title: "Réparation impossible", message: errorMessage(e) });
    },
  });

  const backup = useMutation({
    mutationFn: () => instancesApi.backupWorlds(instance.id),
    onSuccess: (path) =>
      path
        ? notify.success({
            title: "Mondes sauvegardés",
            action: { label: "Ouvrir", onClick: () => instancesApi.openFolder(instance.id, "backups") },
          })
        : notify.info({ title: "Aucun monde à sauvegarder", history: false }),
    onError: (e) => notify.error({ title: "Sauvegarde impossible", message: errorMessage(e) }),
  });

  const spin = (pending: boolean) => pending && <Loader2 className="animate-spin" aria-hidden="true" />;

  return (
    <>
      <SettingSection title="Outils">
        <SettingRow
          label="Dupliquer"
          description="Copie complète (mods, réglages et mondes) sous un nouveau nom."
          control={
            <Button variant="outline" size="sm" className="gap-1.5" onClick={() => setDuplicateOpen(true)}>
              <Copy aria-hidden="true" />
              Dupliquer
            </Button>
          }
        />
        <SettingRow
          label="Exporter en .mrpack"
          description="Partage l'instance : importable dans Largy, Prism, Modrinth App…"
          control={
            <div className="flex items-center gap-3">
              <label className="flex items-center gap-2 text-xs text-muted-foreground">
                <Switch checked={includeSaves} onCheckedChange={setIncludeSaves} />
                Inclure les mondes
              </label>
              <Button
                variant="outline"
                size="sm"
                className="gap-1.5"
                disabled={exportPack.isPending}
                onClick={() => exportPack.mutate()}
              >
                {spin(exportPack.isPending) || <PackageOpen aria-hidden="true" />}
                Exporter
              </Button>
            </div>
          }
        />
        <SettingRow
          label="Réparer"
          description="Vérifie chaque fichier du jeu et du mod loader, et retélécharge ce qui est abîmé."
          control={
            <Button
              variant="outline"
              size="sm"
              className="gap-1.5"
              disabled={repair.isPending || busy}
              onClick={() => repair.mutate()}
            >
              {spin(repair.isPending) || <Wrench aria-hidden="true" />}
              Réparer
            </Button>
          }
        />
        <SettingRow
          label="Sauvegarder les mondes"
          description="Archive le dossier saves (les 5 dernières sauvegardes sont conservées)."
          control={
            <div className="flex items-center gap-2">
              <Button
                variant="ghost"
                size="icon-sm"
                title="Ouvrir les sauvegardes"
                aria-label="Ouvrir les sauvegardes"
                onClick={() => instancesApi.openFolder(instance.id, "backups")}
              >
                <FolderArchive aria-hidden="true" />
              </Button>
              <Button
                variant="outline"
                size="sm"
                className="gap-1.5"
                disabled={backup.isPending}
                onClick={() => backup.mutate()}
              >
                {spin(backup.isPending) || <Archive aria-hidden="true" />}
                Sauvegarder
              </Button>
            </div>
          }
        />
      </SettingSection>

      <Dialog open={duplicateOpen} onOpenChange={setDuplicateOpen}>
        <DialogContent className="sm:max-w-md">
          <DialogHeader>
            <DialogTitle>Dupliquer l'instance</DialogTitle>
            <DialogDescription>Tout le contenu de « {instance.name} » sera copié.</DialogDescription>
          </DialogHeader>
          <Input maxLength={64} value={copyName} onChange={(e) => setCopyName(e.target.value)} autoFocus />
          <DialogFooter>
            <Button variant="outline" onClick={() => setDuplicateOpen(false)}>
              Annuler
            </Button>
            <Button
              disabled={!copyName.trim() || duplicate.isPending}
              className="gap-1.5"
              onClick={() => duplicate.mutate()}
            >
              {spin(duplicate.isPending)}
              Dupliquer
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </>
  );
}
