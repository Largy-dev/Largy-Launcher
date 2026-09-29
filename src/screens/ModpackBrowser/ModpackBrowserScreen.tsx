import { useMemo, useState } from "react";
import { useInfiniteQuery, useQuery } from "@tanstack/react-query";
import { motion } from "motion/react";
import { Link2, Loader2, PackageSearch, Search, X } from "lucide-react";

import { EmptyState } from "@/components/EmptyState";
import { PageHeader } from "@/components/PageHeader";
import { Skeleton } from "@/components/Skeleton";
import { LOADER_META } from "@/components/instance/LoaderBadge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { useDebounced } from "@/hooks/useDebounced";
import { useCurseforgeEnabled } from "@/hooks/useSettings";
import { cn } from "@/lib/utils";
import {
  errorMessage,
  minecraftApi,
  providersApi,
  type LoaderKind,
  type ModpackSummary,
  type ProviderId,
  type SearchSort,
} from "@/services/tauri";

import { JoinPackDialog } from "./JoinPackDialog";
import { ModpackCard } from "./ModpackCard";
import { ModpackDetailDialog } from "./ModpackDetailDialog";

const PROVIDERS: { id: ProviderId; label: string; color: string; pageSize: number }[] = [
  { id: "modrinth", label: "Modrinth", color: "#1bd96a", pageSize: 24 },
  { id: "ftb", label: "FTB", color: "#e5484d", pageSize: 0 },
  { id: "curseforge", label: "CurseForge", color: "#f16436", pageSize: 25 },
];

const SORTS: { value: SearchSort; label: string }[] = [
  { value: "relevance", label: "Populaires" },
  { value: "downloads", label: "Plus téléchargés" },
  { value: "updated", label: "Mis à jour récemment" },
  { value: "newest", label: "Nouveautés" },
];

const LOADERS: LoaderKind[] = ["fabric", "forge", "neoforge", "quilt"];
const ANY = "__any";

interface Filters {
  text: string;
  gameVersion: string | null;
  loader: LoaderKind | null;
  category: string | null;
  sort: SearchSort;
}

const NO_FILTERS: Filters = { text: "", gameVersion: null, loader: null, category: null, sort: "relevance" };

function FilterBar({
  provider,
  filters,
  onChange,
}: {
  provider: ProviderId;
  filters: Filters;
  onChange: (patch: Partial<Filters>) => void;
}) {
  const versions = useQuery({
    queryKey: ["minecraft-versions"],
    queryFn: minecraftApi.listVersions,
    staleTime: 3600_000,
  });
  const releases = useMemo(
    () =>
      (versions.data ?? [])
        .filter((v) => v.type === "release")
        .map((v) => v.id)
        .slice(0, 40),
    [versions.data],
  );
  const categories = useQuery({
    queryKey: ["pack-categories", provider],
    queryFn: () => providersApi.categories(provider),
    staleTime: 3600_000,
  });
  const active = filters.gameVersion || filters.loader || filters.category || filters.sort !== "relevance";

  return (
    <div className="mb-5 space-y-3">
      <div className="flex gap-2">
        <div className="relative flex-1">
          <Search
            className="pointer-events-none absolute top-1/2 left-4 size-4 -translate-y-1/2 text-muted-foreground"
            aria-hidden="true"
          />
          <Input
            type="search"
            placeholder="Rechercher un modpack…"
            aria-label="Rechercher un modpack"
            className="glass h-11 rounded-xl pl-11! text-base"
            value={filters.text}
            onChange={(e) => onChange({ text: e.target.value })}
          />
        </div>
        <Select value={filters.sort} onValueChange={(v) => onChange({ sort: v as SearchSort })}>
          <SelectTrigger size="sm" className="h-11! w-52 rounded-xl" aria-label="Trier">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            {SORTS.map((s) => (
              <SelectItem key={s.value} value={s.value}>
                {s.label}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      </div>
      <div className="flex flex-wrap items-center gap-2">
        <div className="glass flex rounded-lg p-0.5" role="group" aria-label="Mod loader">
          {[null, ...LOADERS].map((loader) => (
            <button
              key={loader ?? "all"}
              aria-pressed={filters.loader === loader}
              onClick={() => onChange({ loader })}
              className={cn(
                "rounded-md px-3 py-1 text-xs font-medium transition-colors",
                filters.loader === loader
                  ? "bg-primary text-primary-foreground"
                  : "text-muted-foreground hover:text-foreground",
              )}
            >
              {loader ? LOADER_META[loader].label : "Tous"}
            </button>
          ))}
        </div>
        <Select
          value={filters.gameVersion ?? ANY}
          onValueChange={(v) => onChange({ gameVersion: v === ANY ? null : v })}
        >
          <SelectTrigger size="sm" className="w-40" aria-label="Version de Minecraft">
            <SelectValue />
          </SelectTrigger>
          <SelectContent className="max-h-72">
            <SelectItem value={ANY}>Toutes les versions</SelectItem>
            {releases.map((v) => (
              <SelectItem key={v} value={v}>
                Minecraft {v}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
        {(categories.data?.length ?? 0) > 0 && (
          <Select value={filters.category ?? ANY} onValueChange={(v) => onChange({ category: v === ANY ? null : v })}>
            <SelectTrigger size="sm" className="w-44" aria-label="Catégorie">
              <SelectValue />
            </SelectTrigger>
            <SelectContent className="max-h-72">
              <SelectItem value={ANY}>Toutes les catégories</SelectItem>
              {categories.data!.map((c) => (
                <SelectItem key={c.id} value={c.id}>
                  {c.label}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        )}

        {active && (
          <Button
            variant="ghost"
            size="sm"
            className="gap-1"
            onClick={() => onChange({ ...NO_FILTERS, text: filters.text })}
          >
            <X aria-hidden="true" />
            Réinitialiser
          </Button>
        )}
      </div>
    </div>
  );
}

function ModpackGrid({
  provider,
  filters,
  onSelect,
}: {
  provider: ProviderId;
  filters: Filters;
  onSelect: (pack: ModpackSummary) => void;
}) {
  const text = useDebounced(filters.text.trim(), 400);
  const pageSize = PROVIDERS.find((p) => p.id === provider)?.pageSize ?? 0;
  const query = {
    text,
    game_version: filters.gameVersion,
    loader: filters.loader,
    category: filters.category,
    sort: filters.sort,
  };

  const {
    data: pages,
    isLoading,
    isError,
    error,
    fetchNextPage,
    hasNextPage,
    isFetchingNextPage,
  } = useInfiniteQuery({
    queryKey: ["modpack-search", provider, query],
    queryFn: ({ pageParam }) => providersApi.search(provider, { ...query, offset: pageParam }),
    initialPageParam: 0,
    getNextPageParam: (last, all) => (pageSize > 0 && last.length >= pageSize ? all.length * pageSize : undefined),
  });
  const data = useMemo(() => {
    const seen = new Set<string>();
    return (pages?.pages.flat() ?? []).filter((p) => !seen.has(p.id) && !!seen.add(p.id));
  }, [pages]);

  return (
    <>
      {isLoading ? (
        <div className="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-3 2xl:grid-cols-4">
          {Array.from({ length: 6 }, (_, i) => (
            <Skeleton key={i} className="h-48 rounded-2xl" />
          ))}
        </div>
      ) : isError ? (
        <EmptyState icon={PackageSearch} title="Erreur" description={errorMessage(error)} />
      ) : !data || data.length === 0 ? (
        <EmptyState
          icon={PackageSearch}
          title="Aucun modpack trouvé"
          description="Essaie un autre terme, ou retire un filtre."
        />
      ) : (
        <motion.div
          key={JSON.stringify(query)}
          initial="hidden"
          animate="show"
          className="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-3 2xl:grid-cols-4"
        >
          {data.map((pack, index) => (
            <ModpackCard key={pack.id} pack={pack} index={index % 24} onSelect={() => onSelect(pack)} />
          ))}
        </motion.div>
      )}
      {hasNextPage && (
        <div className="mt-5 flex justify-center">
          <Button variant="outline" onClick={() => fetchNextPage()} disabled={isFetchingNextPage} className="gap-1.5">
            {isFetchingNextPage && <Loader2 className="animate-spin" aria-hidden="true" />}
            Charger plus
          </Button>
        </div>
      )}
    </>
  );
}

export function ModpackBrowserScreen() {
  const [provider, setProvider] = useState<ProviderId>("modrinth");
  const [selected, setSelected] = useState<ModpackSummary | null>(null);
  const [filters, setFilters] = useState<Filters>(NO_FILTERS);
  const [joinOpen, setJoinOpen] = useState(false);
  const curseforgeEnabled = useCurseforgeEnabled();
  const providers = curseforgeEnabled ? PROVIDERS : PROVIDERS.filter((p) => p.id !== "curseforge");
  const active = providers.some((p) => p.id === provider) ? provider : "modrinth";

  return (
    <div className="flex flex-1 flex-col">
      <PageHeader
        eyebrow="Découvrir"
        title="Modpacks"
        description={
          curseforgeEnabled
            ? "Des milliers d'aventures prêtes à jouer, installées en un clic."
            : "Des milliers de modpacks prêts à jouer. Ajoute une clé CurseForge dans Paramètres › Avancé pour en débloquer plus."
        }
        action={
          <>
            <Button variant="outline" size="sm" className="gap-1.5" onClick={() => setJoinOpen(true)}>
              <Link2 aria-hidden="true" />
              Rejoindre par lien
            </Button>
            <div className="glass flex rounded-xl p-1" role="tablist" aria-label="Catalogue">
              {providers.map((p) => (
                <button
                  key={p.id}
                  role="tab"
                  aria-selected={active === p.id}
                  onClick={() => {
                    setProvider(p.id);
                    setFilters((f) => ({ ...f, category: null }));
                  }}
                  className={cn(
                    "relative rounded-lg px-4 py-1.5 text-sm font-semibold transition-colors",
                    active === p.id ? "text-white" : "text-muted-foreground hover:text-foreground",
                  )}
                >
                  {active === p.id && (
                    <motion.span
                      layoutId="provider-tab"
                      className="absolute inset-0 rounded-lg"
                      style={{ backgroundColor: p.color }}
                    />
                  )}
                  <span className="relative">{p.label}</span>
                </button>
              ))}
            </div>
          </>
        }
      />

      <FilterBar provider={active} filters={filters} onChange={(patch) => setFilters((f) => ({ ...f, ...patch }))} />
      <ModpackGrid provider={active} filters={filters} onSelect={setSelected} />

      <ModpackDetailDialog
        provider={active}
        pack={selected}
        onOpenChange={(open) => !open && setSelected(null)}
        onSwitchPack={(next, pack) => {
          setProvider(next);
          setSelected(pack);
        }}
      />
      <JoinPackDialog open={joinOpen} onOpenChange={setJoinOpen} />
    </div>
  );
}
