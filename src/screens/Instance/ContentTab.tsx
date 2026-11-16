import { useState } from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { open } from "@tauri-apps/plugin-dialog";
import { Compass, FolderOpen, Gauge, Loader2, Plus, RefreshCw } from "lucide-react";

import { ChoiceGroup } from "@/components/settings/SettingsKit";
import { Button } from "@/components/ui/button";
import { notify } from "@/lib/notify";
import { installedApi } from "@/services/content";
import { errorMessage, instancesApi, type ContentKind, type Instance } from "@/services/tauri";
import { InstalledList } from "@/screens/InstanceContent/InstalledList";
import { KIND_META } from "@/screens/InstanceContent/kinds";
import { ModUpdatesDialog } from "@/screens/InstanceContent/ModUpdatesDialog";
import { OptimizeDialog } from "@/screens/InstanceContent/OptimizeDialog";

interface ContentTabProps {
  instance: Instance;
  /** One kind (Mods), or several shown behind a switch (Packs: resource packs + shaders). */
  kinds: ContentKind[];
  onBrowse: (kind: ContentKind, query?: string) => void;
}

/** Installed mods / packs of the instance, with the catalogue and the maintenance tools one click away. */
export function ContentTab({ instance, kinds, onBrowse }: ContentTabProps) {
  const queryClient = useQueryClient();
  const [kind, setKind] = useState<ContentKind>(kinds[0]);
  const [updatesOpen, setUpdatesOpen] = useState(false);
  const [optimizeOpen, setOptimizeOpen] = useState(false);
  const meta = KIND_META[kind];

  const add = useMutation({
    mutationFn: (paths: string[]) => installedApi.add(instance.id, kind, paths),
    onSuccess: (added) => {
      queryClient.invalidateQueries({ queryKey: ["installed", instance.id] });
      queryClient.invalidateQueries({ queryKey: ["content-summary", instance.id] });
      notify.success({
        title: added.length > 1 ? `${added.length} fichiers ajoutés` : "Fichier ajouté",
        history: false,
      });
    },
    onError: (e) => notify.error({ title: "Ajout impossible", message: errorMessage(e) }),
  });

  async function pickFiles() {
    const picked = await open({
      multiple: true,
      filters: [{ name: `${meta.label} (.${meta.extension})`, extensions: [meta.extension] }],
    });
    const paths = Array.isArray(picked) ? picked : typeof picked === "string" ? [picked] : [];
    if (paths.length > 0) add.mutate(paths);
  }

  return (
    <>
      <div className="mb-4 flex flex-wrap items-center gap-2">
        {kinds.length > 1 && (
          <ChoiceGroup
            value={kind}
            onChange={setKind}
            options={kinds.map((k) => ({ value: k, label: KIND_META[k].label, icon: KIND_META[k].icon }))}
          />
        )}
        <div className="flex-1" />
        {kind === "mod" && (
          <>
            <Button variant="ghost" size="sm" className="gap-1.5" onClick={() => setUpdatesOpen(true)}>
              <RefreshCw aria-hidden="true" />
              Mises à jour
            </Button>
            <Button variant="ghost" size="sm" className="gap-1.5" onClick={() => setOptimizeOpen(true)}>
              <Gauge aria-hidden="true" />
              Optimiser
            </Button>
          </>
        )}
        <Button
          variant="ghost"
          size="sm"
          className="gap-1.5"
          onClick={() => instancesApi.openFolder(instance.id, meta.folder)}
        >
          <FolderOpen aria-hidden="true" />
          Dossier
        </Button>
        <Button variant="ghost" size="sm" className="gap-1.5" onClick={pickFiles} disabled={add.isPending}>
          {add.isPending ? <Loader2 className="animate-spin" aria-hidden="true" /> : <Plus aria-hidden="true" />}
          Fichier .{meta.extension}
        </Button>
        <Button size="sm" className="gap-1.5" onClick={() => onBrowse(kind)}>
          <Compass aria-hidden="true" />
          Parcourir
        </Button>
      </div>

      <InstalledList key={kind} instance={instance} kind={kind} onBrowse={(query) => onBrowse(kind, query)} />

      {kind === "mod" && (
        <>
          <ModUpdatesDialog instanceId={instance.id} open={updatesOpen} onOpenChange={setUpdatesOpen} />
          <OptimizeDialog instance={instance} open={optimizeOpen} onOpenChange={setOptimizeOpen} />
        </>
      )}
    </>
  );
}
