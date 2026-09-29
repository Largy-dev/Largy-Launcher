import { useMemo, useState } from "react";
import { useQueries, useQuery } from "@tanstack/react-query";
import { motion } from "motion/react";
import {
  ArrowRightLeft,
  FileDown,
  LayoutGrid,
  LayoutList,
  Loader2,
  type LucideIcon,
  Plus,
  Rows3,
  Search,
  Sparkles,
} from "lucide-react";

import { EmptyState } from "@/components/EmptyState";
import { PageHeader } from "@/components/PageHeader";
import { TipsCard } from "@/components/TipsCard";
import { Skeleton } from "@/components/Skeleton";
import { InstanceHero } from "@/components/instance/InstanceHero";
import { LOADER_META, LOADER_ORDER } from "@/components/instance/LoaderBadge";
import { useFeaturedInstance } from "@/components/shell/AmbientBackground";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Input } from "@/components/ui/input";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { useFileDrop } from "@/hooks/useFileDrop";
import { useImportInstance } from "@/hooks/useImportInstance";
import { formatDuration } from "@/lib/format";
import { cn } from "@/lib/utils";
import { installedApi } from "@/services/content";
import { externalApi } from "@/services/external";
import { instancesApi, type Instance, type LoaderKind } from "@/services/tauri";
import { usePreferences, type CardDensity, type InstanceSort } from "@/store/preferencesStore";

import { CreateInstanceDialog } from "./CreateInstanceDialog";
import { ExternalImportDialog } from "./ExternalImportDialog";
import { InstanceCard } from "./InstanceCard";

const DENSITIES: { value: CardDensity; label: string; icon: LucideIcon }[] = [
  { value: "grid", label: "Grille", icon: LayoutGrid },
  { value: "compact", label: "Compacte", icon: Rows3 },
  { value: "list", label: "Liste", icon: LayoutList },
];

const SORTS: Record<InstanceSort, { label: string; compare: (a: Instance, b: Instance) => number }> = {
  recent: {
    label: "Récemment jouées",
    compare: (a, b) => (b.last_played_at ?? b.created_at) - (a.last_played_at ?? a.created_at),
  },
  name: { label: "Nom", compare: (a, b) => a.name.localeCompare(b.name, "fr") },
  playtime: { label: "Temps de jeu", compare: (a, b) => b.play_time_seconds - a.play_time_seconds },
};

/** One quiet line under the section title: how many instances, hours and mods. */
function StatsLine({ instances }: { instances: Instance[] }) {
  const modded = instances.filter((i) => i.loader !== "vanilla");
  const summaries = useQueries({
    queries: modded.map((i) => ({
      queryKey: ["content-summary", i.id],
      queryFn: () => installedApi.summary(i.id),
      staleTime: 60_000,
    })),
  });
  const totalMods = summaries.reduce((n, q) => n + (q.data?.mods_enabled ?? 0), 0);
  const totalPlay = instances.reduce((n, i) => n + i.play_time_seconds, 0);
  const parts = [
    `${instances.length} instance${instances.length > 1 ? "s" : ""}`,
    totalPlay > 0 ? `${formatDuration(totalPlay)} de jeu` : null,
    totalMods > 0 ? `${totalMods} mods` : null,
  ].filter(Boolean);
  return <p className="text-xs text-muted-foreground tabular-nums">{parts.join(" · ")}</p>;
}

/** First run: instances already sitting in other launchers, one click away. */
function ExternalFoundBanner({ onOpen }: { onOpen: () => void }) {
  const { data } = useQuery({ queryKey: ["external-instances"], queryFn: externalApi.detect, staleTime: 60_000 });
  const count = data?.filter((e) => !e.already_imported).length ?? 0;
  if (count === 0) return null;
  return (
    <button
      type="button"
      onClick={onOpen}
      className="glass mb-4 flex w-full items-center gap-3 rounded-2xl p-4 text-left transition-colors hover:bg-primary/5"
    >
      <div className="bg-gradient-brand flex size-10 shrink-0 items-center justify-center rounded-xl shadow-glow">
        <ArrowRightLeft className="size-5 text-primary-foreground" aria-hidden="true" />
      </div>
      <div className="min-w-0 flex-1">
        <p className="font-semibold">
          {count} instance{count > 1 ? "s" : ""} trouvée{count > 1 ? "s" : ""} dans tes autres launchers
        </p>
        <p className="text-sm text-muted-foreground">
          Récupère tes mods, réglages et mondes en quelques clics — sans rien toucher à l'original.
        </p>
      </div>
      <span className="text-sm font-semibold text-primary">Importer</span>
    </button>
  );
}

export function InstanceListScreen() {
  const [createOpen, setCreateOpen] = useState(false);
  const [externalOpen, setExternalOpen] = useState(false);
  const [search, setSearch] = useState("");
  const [loaderFilter, setLoaderFilter] = useState<LoaderKind | "all">("all");
  const density = usePreferences((s) => s.cardDensity);
  const sort = usePreferences((s) => s.instanceSort);
  const setPrefs = usePreferences((s) => s.set);
  const featured = useFeaturedInstance();
  // Refreshed by the backend's `instances-changed` event (see useAppEvents).
  const { data: instances, isLoading } = useQuery({ queryKey: ["instances"], queryFn: instancesApi.list });
  const { pickAndImport, importPaths, importing } = useImportInstance();
  const dragging = useFileDrop(importPaths);

  const loadersPresent = useMemo(() => LOADER_ORDER.filter((l) => instances?.some((i) => i.loader === l)), [instances]);
  const visible = useMemo(() => {
    const needle = search.trim().toLowerCase();
    return [...(instances ?? [])]
      .filter((i) => loaderFilter === "all" || i.loader === loaderFilter)
      .filter((i) => !needle || i.name.toLowerCase().includes(needle) || i.minecraft_version.includes(needle))
      .sort((a, b) => Number(b.pinned) - Number(a.pinned) || SORTS[sort].compare(a, b));
  }, [instances, loaderFilter, search, sort]);

  const newButton = (
    <div className="flex gap-2">
      <DropdownMenu>
        <DropdownMenuTrigger asChild>
          <Button variant="outline" disabled={importing} className="gap-1.5">
            {importing ? <Loader2 className="animate-spin" aria-hidden="true" /> : <FileDown aria-hidden="true" />}
            Importer
          </Button>
        </DropdownMenuTrigger>
        <DropdownMenuContent align="end" className="w-72">
          <DropdownMenuItem className="gap-2" onClick={pickAndImport}>
            <FileDown className="size-3.5" aria-hidden="true" />
            <span>
              Un fichier…
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
        </DropdownMenuContent>
      </DropdownMenu>
      <Button onClick={() => setCreateOpen(true)} className="gap-1.5">
        <Plus aria-hidden="true" />
        Nouvelle instance
      </Button>
    </div>
  );

  return (
    <div className="flex flex-1 flex-col">
      {isLoading ? (
        <div className="space-y-4">
          <Skeleton className="h-44 rounded-2xl" />
          <div className="grid grid-cols-3 gap-3">
            {[0, 1, 2].map((i) => (
              <Skeleton key={i} className="h-56 rounded-2xl" />
            ))}
          </div>
        </div>
      ) : !instances || instances.length === 0 ? (
        <>
          <PageHeader eyebrow="Bienvenue" title="Prêt pour l'aventure ?" />
          <ExternalFoundBanner onOpen={() => setExternalOpen(true)} />
          <EmptyState
            icon={Sparkles}
            title="Aucune instance pour le moment"
            description="Crée une instance en deux clics, installe un modpack depuis l'onglet Modpacks, ou glisse un fichier .mrpack / .zip ici."
            action={newButton}
          />
        </>
      ) : (
        <>
          {featured && <InstanceHero instance={featured} />}
          <TipsCard />

          <div className="mb-3 flex flex-wrap items-end justify-between gap-2">
            <div>
              <h2 className="text-lg leading-tight font-bold">Mes instances</h2>
              <StatsLine instances={instances} />
            </div>
            {newButton}
          </div>

          <div className="mb-4 flex flex-wrap items-center gap-2">
            {loadersPresent.length > 1 && (
              <div className="flex flex-wrap gap-1.5" role="group" aria-label="Filtrer par mod loader">
                {(["all", ...loadersPresent] as const).map((loader) => {
                  const active = loaderFilter === loader;
                  const color = loader === "all" ? "var(--accent-base)" : LOADER_META[loader].color;
                  return (
                    <button
                      key={loader}
                      aria-pressed={active}
                      onClick={() => setLoaderFilter(loader)}
                      className="rounded-full px-3 py-1 text-xs font-semibold transition-all"
                      style={{
                        color: active ? "white" : `color-mix(in oklab, ${color} 80%, var(--foreground))`,
                        backgroundColor: active ? color : `color-mix(in oklab, ${color} 12%, transparent)`,
                      }}
                    >
                      {loader === "all" ? "Toutes" : LOADER_META[loader].label}
                    </button>
                  );
                })}
              </div>
            )}
            <div className="ml-auto flex items-center gap-2">
              <div className="relative">
                <Search
                  className="pointer-events-none absolute top-1/2 left-2.5 size-3.5 -translate-y-1/2 text-muted-foreground"
                  aria-hidden="true"
                />
                <Input
                  type="search"
                  placeholder="Rechercher…"
                  aria-label="Rechercher une instance"
                  value={search}
                  onChange={(e) => setSearch(e.target.value)}
                  className="h-8 w-40 pl-8!"
                />
              </div>
              <Select value={sort} onValueChange={(v) => setPrefs({ instanceSort: v as InstanceSort })}>
                <SelectTrigger size="sm" className="w-40" aria-label="Trier les instances">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  {Object.entries(SORTS).map(([value, { label }]) => (
                    <SelectItem key={value} value={value}>
                      {label}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
              <div className="glass flex rounded-lg p-0.5">
                {DENSITIES.map(({ value, label, icon: Icon }) => (
                  <button
                    key={value}
                    title={label}
                    aria-label={`Affichage ${label.toLowerCase()}`}
                    aria-pressed={density === value}
                    onClick={() => setPrefs({ cardDensity: value })}
                    className={cn(
                      "flex size-7 items-center justify-center rounded-md text-muted-foreground transition-colors",
                      density === value ? "bg-primary text-primary-foreground" : "hover:text-foreground",
                    )}
                  >
                    <Icon className="size-3.5" aria-hidden="true" />
                  </button>
                ))}
              </div>
            </div>
          </div>

          {visible.length === 0 ? (
            <p className="py-10 text-center text-sm text-muted-foreground">Aucune instance ne correspond.</p>
          ) : (
            <motion.div
              key={`${density}-${loaderFilter}`}
              initial="hidden"
              animate="show"
              className={cn(
                "grid gap-3",
                density === "grid" && "grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 2xl:grid-cols-4",
                density === "compact" && "grid-cols-1 md:grid-cols-2 xl:grid-cols-3",
                density === "list" && "grid-cols-1",
              )}
            >
              {visible.map((instance, index) => (
                <InstanceCard key={instance.id} instance={instance} density={density} index={index} />
              ))}
            </motion.div>
          )}
        </>
      )}

      {dragging && (
        <div className="pointer-events-none fixed inset-4 z-50 flex items-center justify-center rounded-3xl border-2 border-dashed border-primary bg-background/80 backdrop-blur-sm">
          <p className="text-lg font-semibold text-primary">Dépose un .mrpack ou un .zip pour l'importer</p>
        </div>
      )}
      <CreateInstanceDialog open={createOpen} onOpenChange={setCreateOpen} />
      <ExternalImportDialog open={externalOpen} onOpenChange={setExternalOpen} />
    </div>
  );
}
