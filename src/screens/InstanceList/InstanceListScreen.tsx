import { useMemo, useState, type ReactNode } from "react";
import { useQueries, useQuery } from "@tanstack/react-query";
import { motion } from "motion/react";
import {
  FileDown,
  LayoutGrid,
  LayoutList,
  Loader2,
  Package,
  Plus,
  Rows3,
  Search,
  Sparkles,
  Timer,
  type LucideIcon,
} from "lucide-react";

import { AnimatedNumber } from "@/components/AnimatedNumber";
import { EmptyState } from "@/components/EmptyState";
import { PageHeader } from "@/components/PageHeader";
import { Skeleton } from "@/components/Skeleton";
import { InstanceHero } from "@/components/instance/InstanceHero";
import { LOADER_META, LOADER_ORDER } from "@/components/instance/LoaderBadge";
import { useFeaturedInstance } from "@/components/shell/AmbientBackground";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { useFileDrop } from "@/hooks/useFileDrop";
import { useImportInstance } from "@/hooks/useImportInstance";
import { formatDuration } from "@/lib/format";
import { cn } from "@/lib/utils";
import { instanceModsApi, instancesApi, type Instance, type LoaderKind } from "@/services/tauri";
import { usePreferences, type CardDensity, type InstanceSort } from "@/store/preferencesStore";

import { CreateInstanceDialog } from "./CreateInstanceDialog";
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

function StatTile({ icon: Icon, label, children }: { icon: LucideIcon; label: string; children: ReactNode }) {
  return (
    <div className="glass flex items-center gap-3 rounded-xl px-4 py-3">
      <div className="bg-gradient-brand flex size-9 items-center justify-center rounded-lg shadow-glow">
        <Icon className="size-4 text-primary-foreground" aria-hidden="true" />
      </div>
      <div>
        <p className="text-lg leading-tight font-bold tabular-nums">{children}</p>
        <p className="text-xs text-muted-foreground">{label}</p>
      </div>
    </div>
  );
}

function StatsStrip({ instances }: { instances: Instance[] }) {
  const modded = instances.filter((i) => i.loader !== "vanilla");
  const modQueries = useQueries({
    queries: modded.map((i) => ({
      queryKey: ["instance-mods", i.id],
      queryFn: () => instanceModsApi.list(i.id),
      staleTime: 60_000,
    })),
  });
  const totalMods = modQueries.reduce((n, q) => n + (q.data?.filter((m) => m.enabled).length ?? 0), 0);
  const totalPlay = instances.reduce((n, i) => n + i.play_time_seconds, 0);

  return (
    <div className="mb-6 grid grid-cols-3 gap-3">
      <StatTile icon={LayoutGrid} label="Instances">
        <AnimatedNumber value={instances.length} />
      </StatTile>
      <StatTile icon={Timer} label="Temps de jeu cumulé">
        {totalPlay > 0 ? formatDuration(totalPlay) : "—"}
      </StatTile>
      <StatTile icon={Package} label="Mods installés">
        <AnimatedNumber value={totalMods} />
      </StatTile>
    </div>
  );
}

export function InstanceListScreen() {
  const [createOpen, setCreateOpen] = useState(false);
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
      <Button
        variant="outline"
        onClick={pickAndImport}
        disabled={importing}
        className="gap-1.5"
        title="Importer un .mrpack, un zip CurseForge ou une instance Prism/MultiMC"
      >
        {importing ? <Loader2 className="animate-spin" aria-hidden="true" /> : <FileDown aria-hidden="true" />}
        Importer
      </Button>
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
          <StatsStrip instances={instances} />

          <div className="mb-4 flex flex-wrap items-center gap-2">
            <h2 className="mr-auto text-lg font-bold">Mes instances</h2>
            <div className="relative">
              <Search
                className="pointer-events-none absolute top-1/2 left-2.5 size-3.5 -translate-y-1/2 text-muted-foreground"
                aria-hidden="true"
              />
              <Input
                type="search"
                placeholder="Rechercher…"
                value={search}
                onChange={(e) => setSearch(e.target.value)}
                className="h-8 w-44 pl-8!"
              />
            </div>
            <Select value={sort} onValueChange={(v) => setPrefs({ instanceSort: v as InstanceSort })}>
              <SelectTrigger size="sm" className="w-44">
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
            {newButton}
          </div>

          {loadersPresent.length > 1 && (
            <div className="mb-4 flex flex-wrap gap-1.5">
              {(["all", ...loadersPresent] as const).map((loader) => {
                const active = loaderFilter === loader;
                const color = loader === "all" ? "var(--accent-base)" : LOADER_META[loader].color;
                return (
                  <button
                    key={loader}
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
    </div>
  );
}
