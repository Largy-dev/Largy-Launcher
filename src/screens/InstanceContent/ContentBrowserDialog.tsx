import { useEffect, useMemo, useState } from "react";
import { useInfiniteQuery, useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { openUrl } from "@tauri-apps/plugin-opener";
import { Check, Download, ExternalLink, Loader2, Search } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { formatCount } from "@/lib/format";
import { notify } from "@/lib/notify";
import { cn } from "@/lib/utils";
import { contentApi, installedApi, type RemoteProvider } from "@/services/content";
import { errorMessage, type ContentHit, type ContentKind, type Instance } from "@/services/tauri";

import { installedQueryKey } from "./InstalledList";
import { PROVIDER_META } from "./InstalledRow";
import { KIND_META } from "./kinds";

const PAGE_SIZE = 30;

interface ContentBrowserDialogProps {
  instance: Instance;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  initialKind: ContentKind;
  /** Pre-filled search (e.g. a missing dependency's id). */
  initialQuery?: string;
}

function useDebounced<T>(value: T, delay: number): T {
  const [debounced, setDebounced] = useState(value);
  useEffect(() => {
    const id = window.setTimeout(() => setDebounced(value), delay);
    return () => window.clearTimeout(id);
  }, [value, delay]);
  return debounced;
}

/**
 * Modrinth and CurseForge catalogues, filtered to what's compatible with this
 * instance. Remount it (`key`) to start over from `initialKind` / `initialQuery`.
 */
export function ContentBrowserDialog({
  instance,
  open,
  onOpenChange,
  initialKind,
  initialQuery,
}: ContentBrowserDialogProps) {
  const queryClient = useQueryClient();
  const kinds = (Object.keys(KIND_META) as ContentKind[]).filter((k) => k !== "mod" || instance.loader !== "vanilla");
  const [kind, setKind] = useState<ContentKind>(initialKind);
  const [provider, setProvider] = useState<RemoteProvider>("modrinth");
  const [text, setText] = useState(initialQuery ?? "");
  const query = useDebounced(text.trim(), 350);
  const [justInstalled, setJustInstalled] = useState<Set<string>>(new Set());

  const curseforge = useQuery({
    queryKey: ["content-curseforge-available"],
    queryFn: contentApi.curseforgeAvailable,
    staleTime: Infinity,
  });
  const providers: RemoteProvider[] = curseforge.data ? ["modrinth", "curseforge"] : ["modrinth"];

  const installed = useQuery({
    queryKey: installedQueryKey(instance.id, kind),
    queryFn: () => installedApi.list(instance.id, kind),
    enabled: open,
  });
  const installedIds = useMemo(
    () =>
      new Set((installed.data ?? []).flatMap((i) => (i.remote ? [`${i.remote.provider}:${i.remote.project_id}`] : []))),
    [installed.data],
  );

  const results = useInfiniteQuery({
    queryKey: ["content-search", instance.id, provider, kind, query],
    queryFn: ({ pageParam }) => contentApi.search(instance.id, provider, kind, query, pageParam),
    initialPageParam: 0,
    getNextPageParam: (last, pages) => (last.length < PAGE_SIZE ? undefined : pages.length * PAGE_SIZE),
    enabled: open,
    staleTime: 60_000,
  });
  const hits = useMemo(() => {
    const seen = new Set<string>();
    return (results.data?.pages.flat() ?? []).filter((h) => !seen.has(h.project_id) && seen.add(h.project_id));
  }, [results.data]);

  const install = useMutation({
    mutationFn: (hit: ContentHit) => contentApi.install(instance.id, hit.provider, hit.project_id, kind),
    onSuccess: (files, hit) => {
      setJustInstalled((prev) => new Set(prev).add(`${hit.provider}:${hit.project_id}`));
      queryClient.invalidateQueries({ queryKey: ["installed", instance.id] });
      queryClient.invalidateQueries({ queryKey: ["content-summary", instance.id] });
      const extra = files.length > 1 ? ` (+ ${files.length - 1} dépendance${files.length > 2 ? "s" : ""})` : "";
      notify.success({ title: `${hit.title} installé${extra}`, history: false });
    },
    onError: (e, hit) => notify.error({ title: `Impossible d'installer ${hit.title}`, message: errorMessage(e) }),
  });

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="flex h-[85vh] flex-col sm:max-w-3xl">
        <DialogHeader>
          <DialogTitle>Ajouter du contenu</DialogTitle>
          <DialogDescription>
            Compatible avec Minecraft {instance.minecraft_version}
            {instance.loader !== "vanilla" && ` · ${instance.loader}`}. Les dépendances requises sont installées
            automatiquement.
          </DialogDescription>
        </DialogHeader>

        <div className="flex flex-wrap items-center gap-2">
          <div className="glass flex rounded-lg p-0.5" role="tablist" aria-label="Type de contenu">
            {kinds.map((k) => (
              <button
                key={k}
                role="tab"
                aria-selected={kind === k}
                onClick={() => setKind(k)}
                className={cn(
                  "rounded-md px-3 py-1 text-xs font-medium transition-colors",
                  kind === k ? "bg-primary text-primary-foreground" : "text-muted-foreground hover:text-foreground",
                )}
              >
                {KIND_META[k].label}
              </button>
            ))}
          </div>
          {providers.length > 1 && (
            <div className="glass flex rounded-lg p-0.5" role="tablist" aria-label="Catalogue">
              {providers.map((p) => (
                <button
                  key={p}
                  role="tab"
                  aria-selected={provider === p}
                  onClick={() => setProvider(p)}
                  className={cn(
                    "flex items-center gap-1.5 rounded-md px-3 py-1 text-xs font-medium transition-colors",
                    provider === p ? "bg-foreground/10 text-foreground" : "text-muted-foreground hover:text-foreground",
                  )}
                >
                  <span className="size-1.5 rounded-full" style={{ backgroundColor: PROVIDER_META[p].color }} />
                  {PROVIDER_META[p].label}
                </button>
              ))}
            </div>
          )}
          <div className="relative ml-auto">
            <Search
              className="pointer-events-none absolute top-1/2 left-2.5 size-3.5 -translate-y-1/2 text-muted-foreground"
              aria-hidden="true"
            />
            <Input
              type="search"
              placeholder="Rechercher…"
              value={text}
              onChange={(e) => setText(e.target.value)}
              className="h-8 w-60 pl-8!"
              autoFocus
            />
          </div>
        </div>

        <div className="-mx-1 min-h-0 flex-1 overflow-y-auto px-1">
          {results.isLoading && (
            <div className="flex justify-center py-12">
              <Loader2 className="size-5 animate-spin text-muted-foreground" aria-hidden="true" />
            </div>
          )}
          {results.isError && (
            <p className="py-8 text-center text-sm text-destructive">{errorMessage(results.error)}</p>
          )}
          {results.isSuccess && hits.length === 0 && (
            <p className="py-8 text-center text-sm text-muted-foreground">Aucun résultat compatible.</p>
          )}
          <ul className="divide-y divide-border/60">
            {hits.map((hit) => {
              const key = `${hit.provider}:${hit.project_id}`;
              const done = installedIds.has(key) || justInstalled.has(key);
              const pending = install.isPending && install.variables?.project_id === hit.project_id;
              return (
                <li key={key} className="flex items-center gap-3 py-2.5">
                  {hit.icon_url ? (
                    <img
                      src={hit.icon_url}
                      alt=""
                      loading="lazy"
                      className="size-11 shrink-0 rounded-lg object-cover"
                    />
                  ) : (
                    <div className="size-11 shrink-0 rounded-lg bg-muted" />
                  )}
                  <div className="min-w-0 flex-1">
                    <p className="flex min-w-0 items-center gap-2 text-sm font-semibold">
                      <span className="truncate">{hit.title}</span>
                      {hit.author && (
                        <span className="shrink-0 text-xs font-normal text-muted-foreground">par {hit.author}</span>
                      )}
                    </p>
                    <p className="line-clamp-1 text-xs text-muted-foreground">{hit.description}</p>
                  </div>
                  <span className="hidden shrink-0 text-xs text-muted-foreground tabular-nums sm:block">
                    {formatCount(hit.downloads)}
                  </span>
                  <Button
                    variant="ghost"
                    size="icon-sm"
                    aria-label={`Ouvrir la page de ${hit.title}`}
                    onClick={() => openUrl(hit.url)}
                  >
                    <ExternalLink aria-hidden="true" />
                  </Button>
                  <Button
                    size="sm"
                    variant={done ? "outline" : "default"}
                    disabled={done || pending}
                    className="w-28 gap-1.5"
                    onClick={() => install.mutate(hit)}
                  >
                    {pending ? (
                      <Loader2 className="animate-spin" aria-hidden="true" />
                    ) : done ? (
                      <Check aria-hidden="true" />
                    ) : (
                      <Download aria-hidden="true" />
                    )}
                    {done ? "Installé" : "Installer"}
                  </Button>
                </li>
              );
            })}
          </ul>
          {results.hasNextPage && (
            <div className="flex justify-center py-4">
              <Button
                variant="outline"
                size="sm"
                disabled={results.isFetchingNextPage}
                onClick={() => results.fetchNextPage()}
                className="gap-1.5"
              >
                {results.isFetchingNextPage && <Loader2 className="animate-spin" aria-hidden="true" />}
                Plus de résultats
              </Button>
            </div>
          )}
        </div>
      </DialogContent>
    </Dialog>
  );
}
