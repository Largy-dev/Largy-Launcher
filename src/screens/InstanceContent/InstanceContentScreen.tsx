import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useNavigate, useParams, useSearchParams } from "react-router";
import { open } from "@tauri-apps/plugin-dialog";
import { ArrowLeft, Compass, FolderOpen, Gauge, Loader2, MoreHorizontal, Plus, RefreshCw } from "lucide-react";

import { PageHeader } from "@/components/PageHeader";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { notify } from "@/lib/notify";
import { cn } from "@/lib/utils";
import { installedApi, type ContentSummary } from "@/services/content";
import { errorMessage, instancesApi, type ContentKind } from "@/services/tauri";

import { ContentBrowserDialog } from "./ContentBrowserDialog";
import { InstalledList } from "./InstalledList";
import { KIND_META, WORLDS_META, type ContentTab } from "./kinds";
import { ModUpdatesDialog } from "./ModUpdatesDialog";
import { OptimizeDialog } from "./OptimizeDialog";
import { WorldsTab } from "./WorldsTab";

const TABS: ContentTab[] = ["mod", "resource_pack", "shader", "worlds"];

function tabCount(summary: ContentSummary | undefined, tab: ContentTab): number | null {
  if (!summary) return null;
  switch (tab) {
    case "mod":
      return summary.mods;
    case "resource_pack":
      return summary.resource_packs;
    case "shader":
      return summary.shaders;
    case "worlds":
      return summary.worlds;
  }
}

/** Everything inside an instance: mods, resource packs, shaders and worlds. */
export function InstanceContentScreen() {
  const { id } = useParams<{ id: string }>();
  const instanceId = id ?? "";
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const [params, setParams] = useSearchParams();
  const [browse, setBrowse] = useState<{ open: boolean; query?: string; nonce: number }>({ open: false, nonce: 0 });
  const openBrowser = (query?: string) => setBrowse((b) => ({ open: true, query, nonce: b.nonce + 1 }));
  const [updatesOpen, setUpdatesOpen] = useState(false);
  const [optimizeOpen, setOptimizeOpen] = useState(false);

  const { data: instance } = useQuery({
    queryKey: ["instance", instanceId],
    queryFn: () => instancesApi.get(instanceId),
    enabled: instanceId !== "",
  });
  const { data: summary } = useQuery({
    queryKey: ["content-summary", instanceId],
    queryFn: () => installedApi.summary(instanceId),
    enabled: instanceId !== "",
  });

  const vanilla = instance?.loader === "vanilla";
  const tabs = TABS.filter((t) => t !== "mod" || !vanilla);
  const requested = params.get("tab") as ContentTab | null;
  const tab: ContentTab = requested && tabs.includes(requested) ? requested : tabs[0];
  const kind: ContentKind | null = tab === "worlds" ? null : tab;

  const add = useMutation({
    mutationFn: (paths: string[]) => installedApi.add(instanceId, kind!, paths),
    onSuccess: (added) => {
      queryClient.invalidateQueries({ queryKey: ["installed", instanceId] });
      queryClient.invalidateQueries({ queryKey: ["content-summary", instanceId] });
      notify.success({
        title: added.length > 1 ? `${added.length} fichiers ajoutés` : "Fichier ajouté",
        history: false,
      });
    },
    onError: (e) => notify.error({ title: "Ajout impossible", message: errorMessage(e) }),
  });

  async function pickFiles() {
    if (!kind) return;
    const meta = KIND_META[kind];
    const picked = await open({
      multiple: true,
      filters: [{ name: `${meta.label} (.${meta.extension})`, extensions: [meta.extension] }],
    });
    const paths = Array.isArray(picked) ? picked : typeof picked === "string" ? [picked] : [];
    if (paths.length > 0) add.mutate(paths);
  }

  const description =
    summary && instance
      ? [
          `Minecraft ${instance.minecraft_version}`,
          !vanilla &&
            `${summary.mods_enabled} mod${summary.mods_enabled > 1 ? "s" : ""} actif${summary.mods_enabled > 1 ? "s" : ""}`,
          summary.worlds === 0 ? "aucun monde" : `${summary.worlds} monde${summary.worlds > 1 ? "s" : ""}`,
        ]
          .filter(Boolean)
          .join(" · ")
      : " ";

  return (
    <div className="flex flex-1 flex-col">
      <PageHeader
        eyebrow={instance?.name ?? "Instance"}
        title="Contenu"
        description={description}
        action={
          <>
            <Button variant="ghost" size="sm" className="gap-1.5" onClick={() => navigate(`/instances/${instanceId}`)}>
              <ArrowLeft aria-hidden="true" />
              Retour
            </Button>
            {kind && (
              <>
                {kind === "mod" && (
                  <>
                    <Button variant="outline" size="sm" className="gap-1.5" onClick={() => setUpdatesOpen(true)}>
                      <RefreshCw aria-hidden="true" />
                      Mises à jour
                    </Button>
                    <Button variant="outline" size="sm" className="gap-1.5" onClick={() => setOptimizeOpen(true)}>
                      <Gauge aria-hidden="true" />
                      Optimiser
                    </Button>
                  </>
                )}
                <DropdownMenu>
                  <DropdownMenuTrigger asChild>
                    <Button variant="outline" size="icon-sm" aria-label="Plus d'actions">
                      {add.isPending ? (
                        <Loader2 className="animate-spin" aria-hidden="true" />
                      ) : (
                        <MoreHorizontal aria-hidden="true" />
                      )}
                    </Button>
                  </DropdownMenuTrigger>
                  <DropdownMenuContent align="end" className="w-56">
                    <DropdownMenuItem className="gap-2" onClick={pickFiles}>
                      <Plus className="size-3.5" aria-hidden="true" />
                      Ajouter un fichier .{KIND_META[kind].extension}…
                    </DropdownMenuItem>
                    <DropdownMenuItem
                      className="gap-2"
                      onClick={() => instancesApi.openFolder(instanceId, KIND_META[kind].folder)}
                    >
                      <FolderOpen className="size-3.5" aria-hidden="true" />
                      Ouvrir le dossier {KIND_META[kind].folder}
                    </DropdownMenuItem>
                  </DropdownMenuContent>
                </DropdownMenu>
                <Button size="sm" onClick={() => openBrowser()} className="bg-gradient-brand gap-1.5">
                  <Compass aria-hidden="true" />
                  Parcourir
                </Button>
              </>
            )}
          </>
        }
      />

      <div className="mb-5 flex gap-1 border-b border-border/70" role="tablist" aria-label="Contenu de l'instance">
        {tabs.map((t) => {
          const meta = t === "worlds" ? WORLDS_META : KIND_META[t];
          const Icon = meta.icon;
          const count = tabCount(summary, t);
          const active = t === tab;
          return (
            <button
              key={t}
              role="tab"
              aria-selected={active}
              onClick={() => setParams({ tab: t }, { replace: true })}
              className={cn(
                "relative -mb-px flex items-center gap-2 px-3.5 py-2.5 text-sm font-medium transition-colors",
                active ? "text-foreground" : "text-muted-foreground hover:text-foreground",
              )}
            >
              <Icon className="size-4" aria-hidden="true" />
              {meta.label}
              {count !== null && (
                <span
                  className={cn(
                    "rounded-full px-1.5 text-[0.7rem] tabular-nums",
                    active ? "bg-primary/15 text-primary" : "bg-muted text-muted-foreground",
                  )}
                >
                  {count}
                </span>
              )}
              {active && (
                <span className="absolute inset-x-2 -bottom-px h-0.5 rounded-full bg-primary" aria-hidden="true" />
              )}
            </button>
          );
        })}
      </div>

      {instance &&
        (kind ? (
          <InstalledList key={kind} instance={instance} kind={kind} onBrowse={openBrowser} />
        ) : (
          <WorldsTab instance={instance} />
        ))}

      {instance && kind && (
        <ContentBrowserDialog
          key={browse.nonce}
          instance={instance}
          open={browse.open}
          onOpenChange={(o) => setBrowse((b) => ({ ...b, open: o }))}
          initialKind={kind}
          initialQuery={browse.query}
        />
      )}
      <ModUpdatesDialog instanceId={instanceId} open={updatesOpen} onOpenChange={setUpdatesOpen} />
      {instance && <OptimizeDialog instance={instance} open={optimizeOpen} onOpenChange={setOptimizeOpen} />}
    </div>
  );
}
