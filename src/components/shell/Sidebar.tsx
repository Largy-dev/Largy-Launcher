import { useMemo, useState } from "react";
import { useMatch, useNavigate } from "react-router";
import { motion } from "motion/react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { ArrowRightLeft, Bug, CodeXml, FileDown, Link2, Loader2, Plus, Search, Sparkles } from "lucide-react";

import { InstanceIcon } from "@/components/instance/InstanceIcon";
import { LOADER_META } from "@/components/instance/LoaderBadge";
import { useInstanceMenu } from "@/components/instance/InstanceMenu";
import { ReportIssueDialog } from "@/components/ReportIssueDialog";
import {
  ContextMenu,
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuSeparator,
  ContextMenuTrigger,
} from "@/components/ui/context-menu";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { useAppVersion } from "@/hooks/useAppVersion";
import { useImportInstance } from "@/hooks/useImportInstance";
import { byRecent, useInstances, useSelectedInstance } from "@/hooks/useInstances";
import { useNow } from "@/hooks/useInstanceInfo";
import { formatClock } from "@/lib/format";
import { REPO_URL } from "@/lib/issueReport";
import { spring } from "@/lib/motion";
import { cn } from "@/lib/utils";
import { JoinPackDialog } from "@/screens/ModpackBrowser/JoinPackDialog";
import { CreateInstanceDialog } from "@/screens/InstanceList/CreateInstanceDialog";
import { ExternalImportDialog } from "@/screens/InstanceList/ExternalImportDialog";
import type { Instance } from "@/services/tauri";
import { runtimeOf, useAppStore } from "@/store/appStore";

import { UpdateCard } from "./UpdateCard";

/** Shows the search box once the list gets long enough to need it. */
const SEARCH_THRESHOLD = 7;

function InstanceItem({ instance, selected }: { instance: Instance; selected: boolean }) {
  const navigate = useNavigate();
  const runtime = useAppStore((s) => runtimeOf(s.runtime, instance.id));
  const installing = useAppStore((s) => s.downloadProgress?.task_id === instance.id);
  const now = useNow(1000, runtime.running && !!runtime.startedAt);
  const menu = useInstanceMenu(instance);
  const preparing = runtime.running && runtime.phase !== "running";

  const subtitle = runtime.running
    ? preparing
      ? "Lancement…"
      : runtime.startedAt
        ? `En jeu · ${formatClock((now - runtime.startedAt) / 1000)}`
        : "En jeu"
    : installing
      ? "Installation…"
      : `${LOADER_META[instance.loader].label} · ${instance.minecraft_version}`;

  return (
    <>
      <ContextMenu>
        <ContextMenuTrigger asChild>
          <button
            onClick={() => navigate(`/instances/${instance.id}`)}
            aria-current={selected ? "page" : undefined}
            className={cn(
              "group relative flex w-full items-center gap-2.5 rounded-xl px-2 py-1.5 text-left transition-colors",
              selected ? "text-sidebar-foreground" : "text-sidebar-foreground/75 hover:bg-sidebar-accent",
            )}
          >
            {selected && (
              <motion.span
                layoutId="sidebar-selected"
                transition={spring}
                className="absolute inset-0 rounded-xl bg-sidebar-accent ring-1 ring-sidebar-border"
              />
            )}
            <span className="relative">
              <InstanceIcon instance={instance} className="size-9 rounded-lg" />
              {runtime.running && (
                <span
                  className="absolute -right-0.5 -bottom-0.5 size-3 animate-pulse-glow rounded-full border-2 border-background bg-success"
                  aria-label="En cours"
                />
              )}
            </span>
            <span className="relative min-w-0 flex-1">
              <span className="block truncate text-sm font-medium">{instance.name}</span>
              <span
                className={cn(
                  "block truncate text-[0.7rem] tabular-nums",
                  runtime.running ? "text-success" : "text-sidebar-foreground/45",
                )}
              >
                {subtitle}
              </span>
            </span>
            {instance.pinned && (
              <span className="relative size-1.5 shrink-0 rounded-full bg-primary/70" title="Épinglée" />
            )}
          </button>
        </ContextMenuTrigger>
        <ContextMenuContent>
          {menu.items({ Item: ContextMenuItem, Separator: ContextMenuSeparator }, { withPlay: true })}
        </ContextMenuContent>
      </ContextMenu>
      {menu.dialogs}
    </>
  );
}

function AddInstanceMenu() {
  const [createOpen, setCreateOpen] = useState(false);
  const [externalOpen, setExternalOpen] = useState(false);
  const [joinOpen, setJoinOpen] = useState(false);
  const { pickAndImport, importing } = useImportInstance();

  return (
    <>
      <DropdownMenu>
        <DropdownMenuTrigger asChild>
          <button
            title="Ajouter une instance"
            aria-label="Ajouter une instance"
            className="flex size-7 items-center justify-center rounded-lg text-sidebar-foreground/60 transition-colors hover:bg-sidebar-accent hover:text-sidebar-foreground"
          >
            {importing ? (
              <Loader2 className="size-4 animate-spin" aria-hidden="true" />
            ) : (
              <Plus className="size-4" aria-hidden="true" />
            )}
          </button>
        </DropdownMenuTrigger>
        <DropdownMenuContent align="end" className="w-72">
          <DropdownMenuItem className="gap-2" onClick={() => setCreateOpen(true)}>
            <Sparkles className="size-3.5" aria-hidden="true" />
            <span>
              Nouvelle instance…
              <span className="block text-xs text-muted-foreground">Vanilla, Fabric, Forge, NeoForge, Quilt</span>
            </span>
          </DropdownMenuItem>
          <DropdownMenuSeparator />
          <DropdownMenuItem className="gap-2" onClick={pickAndImport}>
            <FileDown className="size-3.5" aria-hidden="true" />
            <span>
              Importer un fichier…
              <span className="block text-xs text-muted-foreground">.mrpack, zip CurseForge, export Prism</span>
            </span>
          </DropdownMenuItem>
          <DropdownMenuItem className="gap-2" onClick={() => setExternalOpen(true)}>
            <ArrowRightLeft className="size-3.5" aria-hidden="true" />
            <span>
              Depuis un autre launcher…
              <span className="block text-xs text-muted-foreground">Officiel, Prism, CurseForge, Modrinth App</span>
            </span>
          </DropdownMenuItem>
          <DropdownMenuItem className="gap-2" onClick={() => setJoinOpen(true)}>
            <Link2 className="size-3.5" aria-hidden="true" />
            <span>
              Rejoindre un pack par lien…
              <span className="block text-xs text-muted-foreground">Un .mrpack partagé par un ami ou un serveur</span>
            </span>
          </DropdownMenuItem>
        </DropdownMenuContent>
      </DropdownMenu>
      <CreateInstanceDialog open={createOpen} onOpenChange={setCreateOpen} />
      <ExternalImportDialog open={externalOpen} onOpenChange={setExternalOpen} />
      <JoinPackDialog open={joinOpen} onOpenChange={setJoinOpen} />
    </>
  );
}

export function Sidebar() {
  const version = useAppVersion();
  const { data: instances, isLoading } = useInstances();
  const selected = useSelectedInstance();
  const atRoot = useMatch("/");
  const atInstance = useMatch("/instances/:id");
  const onHome = atRoot !== null || atInstance !== null;
  const [search, setSearch] = useState("");
  const [reporting, setReporting] = useState(false);

  const visible = useMemo(() => {
    const needle = search.trim().toLowerCase();
    return [...(instances ?? [])]
      .filter((i) => !needle || i.name.toLowerCase().includes(needle) || i.minecraft_version.includes(needle))
      .sort((a, b) => Number(b.pinned) - Number(a.pinned) || byRecent(a, b));
  }, [instances, search]);

  return (
    <aside className="z-10 flex w-64 shrink-0 flex-col border-r border-sidebar-border bg-sidebar text-sidebar-foreground backdrop-blur-2xl">
      <div className="flex items-center gap-2 px-4 pt-4 pb-2">
        <p className="flex-1 text-sm font-semibold">
          Instances
          {instances && instances.length > 0 && (
            <span className="ml-1.5 text-xs font-normal text-sidebar-foreground/40 tabular-nums">
              {instances.length}
            </span>
          )}
        </p>
        <AddInstanceMenu />
      </div>

      {(instances?.length ?? 0) >= SEARCH_THRESHOLD && (
        <div className="relative mx-3 mb-1">
          <Search
            className="pointer-events-none absolute top-1/2 left-2.5 size-3.5 -translate-y-1/2 text-sidebar-foreground/40"
            aria-hidden="true"
          />
          <input
            type="search"
            placeholder="Rechercher…"
            aria-label="Rechercher une instance"
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            className="h-8 w-full rounded-lg bg-sidebar-accent pr-2 pl-8 text-sm outline-none placeholder:text-sidebar-foreground/40 focus:ring-2 focus:ring-ring/40"
          />
        </div>
      )}

      <nav aria-label="Instances" className="flex-1 space-y-0.5 overflow-y-auto px-2 py-1">
        {isLoading &&
          [0, 1, 2].map((i) => <div key={i} className="mx-1 my-1 h-11 animate-pulse rounded-xl bg-sidebar-accent" />)}
        {visible.map((instance) => (
          <InstanceItem key={instance.id} instance={instance} selected={onHome && selected?.id === instance.id} />
        ))}
        {instances && instances.length > 0 && visible.length === 0 && (
          <p className="px-2 py-4 text-center text-xs text-sidebar-foreground/45">Aucune instance ne correspond.</p>
        )}
      </nav>

      <div className="space-y-2 p-3">
        <UpdateCard />
        <div className="space-y-0.5">
          <button
            onClick={() => setReporting(true)}
            className="flex w-full items-center gap-2.5 rounded-lg px-2 py-1.5 text-sm text-sidebar-foreground/65 transition-colors hover:bg-sidebar-accent hover:text-sidebar-foreground"
          >
            <Bug className="size-4" aria-hidden="true" />
            Signaler un problème
          </button>
          <button
            onClick={() => openUrl(REPO_URL)}
            className="flex w-full items-center gap-2.5 rounded-lg px-2 py-1.5 text-sm text-sidebar-foreground/65 transition-colors hover:bg-sidebar-accent hover:text-sidebar-foreground"
          >
            <CodeXml className="size-4" aria-hidden="true" />
            Code source
          </button>
        </div>
        <p className="px-2 text-[0.68rem] text-sidebar-foreground/35">Largy Launcher {version ? `v${version}` : ""}</p>
      </div>
      <ReportIssueDialog open={reporting} onOpenChange={setReporting} />
    </aside>
  );
}
