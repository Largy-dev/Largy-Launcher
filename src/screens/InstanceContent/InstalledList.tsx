import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useVirtualizer } from "@tanstack/react-virtual";
import { AlertTriangle, Compass, Loader2, Search } from "lucide-react";

import { ConfirmDialog } from "@/components/ConfirmDialog";
import { EmptyState } from "@/components/EmptyState";
import { Skeleton } from "@/components/Skeleton";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { useFileDrop } from "@/hooks/useFileDrop";
import { useScrollContainer } from "@/hooks/useScrollContainer";
import { analyseDependencies, displayName, matchesSearch, problemFiles } from "@/lib/installedContent";
import { notify } from "@/lib/notify";
import { cn } from "@/lib/utils";
import { installedApi, type InstalledItem } from "@/services/content";
import { errorMessage, instancesApi, type ContentKind, type Instance } from "@/services/tauri";

import { KIND_META } from "./kinds";
import { InstalledRow, ROW_HEIGHT } from "./InstalledRow";
import { SelectionBar } from "./SelectionBar";

type Filter = "all" | "enabled" | "disabled" | "problems";
type Sort = "name" | "recent" | "size";

const SORTS: Record<Sort, { label: string; compare: (a: InstalledItem, b: InstalledItem) => number }> = {
  name: { label: "Nom", compare: (a, b) => displayName(a).localeCompare(displayName(b), "fr") },
  recent: { label: "Récents", compare: (a, b) => b.modified - a.modified },
  size: { label: "Taille", compare: (a, b) => b.size - a.size },
};

export function installedQueryKey(instanceId: string, kind: ContentKind) {
  return ["installed", instanceId, kind] as const;
}

interface InstalledListProps {
  instance: Instance;
  kind: ContentKind;
  onBrowse: (query?: string) => void;
}

/** Everything installed for one content kind, with bulk actions and dependency checks. */
export function InstalledList({ instance, kind, onBrowse }: InstalledListProps) {
  const meta = KIND_META[kind];
  const queryClient = useQueryClient();
  const queryKey = installedQueryKey(instance.id, kind);
  const [search, setSearch] = useState("");
  const [filter, setFilter] = useState<Filter>("all");
  const [sort, setSort] = useState<Sort>("name");
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [anchor, setAnchor] = useState<string | null>(null);
  const [toDelete, setToDelete] = useState<InstalledItem[] | null>(null);
  const [toDisable, setToDisable] = useState<{ item: InstalledItem; requiredBy: string[] } | null>(null);

  const list = useQuery({ queryKey, queryFn: () => installedApi.list(instance.id, kind) });
  const items = useMemo(() => list.data ?? [], [list.data]);

  // Look up unidentified files on Modrinth / CurseForge in the background.
  const needsLookup = items.some((i) => !i.is_dir && !i.remote);
  const identify = useQuery({
    queryKey: ["installed-identify", instance.id, kind],
    queryFn: () => installedApi.identify(instance.id, kind),
    enabled: list.isSuccess && needsLookup,
    staleTime: 10 * 60_000,
    retry: false,
  });
  useEffect(() => {
    if (identify.data) queryClient.invalidateQueries({ queryKey });
    // queryKey is rebuilt every render; its parts are the real dependencies.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [identify.data, queryClient, instance.id, kind]);

  const deps = useMemo(() => (kind === "mod" ? analyseDependencies(items) : null), [items, kind]);
  const problemSet = useMemo(() => (deps ? problemFiles(deps) : new Set<string>()), [deps]);
  const problems = problemSet.size;
  const enabledCount = items.filter((i) => i.enabled).length;

  const visible = useMemo(
    () =>
      items
        .filter((i) => {
          if (filter === "enabled") return i.enabled;
          if (filter === "disabled") return !i.enabled;
          if (filter === "problems") return problemSet.has(i.file_name);
          return true;
        })
        .filter((i) => matchesSearch(i, search))
        .sort(SORTS[sort].compare),
    [items, filter, search, sort, problemSet],
  );

  // Drop the selection of files that disappeared.
  useEffect(() => {
    setSelected((prev) => {
      const names = new Set(items.map((i) => i.file_name));
      const next = new Set([...prev].filter((n) => names.has(n)));
      return next.size === prev.size ? prev : next;
    });
  }, [items]);

  const invalidate = useCallback(() => {
    queryClient.invalidateQueries({ queryKey: ["installed", instance.id] });
    queryClient.invalidateQueries({ queryKey: ["content-summary", instance.id] });
  }, [queryClient, instance.id]);

  const reportFailures = (failed: string[], title: string) => {
    if (failed.length > 0) notify.warning({ title, message: failed.slice(0, 6).join("\n") });
  };

  const setEnabled = useMutation({
    mutationFn: (vars: { names: string[]; enabled: boolean }) =>
      installedApi.setEnabled(instance.id, kind, vars.names, vars.enabled),
    onSuccess: (failed) => {
      invalidate();
      reportFailures(failed, "Certains fichiers n'ont pas pu être modifiés");
    },
    onError: (e) => notify.error({ title: "Action impossible", message: errorMessage(e) }),
  });

  const remove = useMutation({
    mutationFn: (names: string[]) => installedApi.delete(instance.id, kind, names),
    onSuccess: (failed, names) => {
      invalidate();
      setToDelete(null);
      setSelected(new Set());
      const done = names.length - failed.length;
      if (done > 0)
        notify.success({
          title: `${done} élément${done > 1 ? "s" : ""} supprimé${done > 1 ? "s" : ""}`,
          history: false,
        });
      reportFailures(failed, "Certains fichiers n'ont pas pu être supprimés");
    },
    onError: (e) => notify.error({ title: "Suppression impossible", message: errorMessage(e) }),
  });

  const add = useMutation({
    mutationFn: (paths: string[]) => installedApi.add(instance.id, kind, paths),
    onSuccess: (added) => {
      invalidate();
      notify.success({
        title: added.length > 1 ? `${added.length} fichiers ajoutés` : "Fichier ajouté",
        history: false,
      });
    },
    onError: (e) => notify.error({ title: "Ajout impossible", message: errorMessage(e) }),
  });

  const onDrop = useCallback(
    (paths: string[]) => {
      const ext = `.${meta.extension}`;
      const accepted = paths.filter((p) => p.toLowerCase().endsWith(ext));
      if (accepted.length > 0) add.mutate(accepted);
      else notify.warning({ title: `Seuls les fichiers ${ext} peuvent être ajoutés ici`, history: false });
    },
    [add, meta.extension],
  );
  const dragging = useFileDrop(onDrop);

  const onSelect = useCallback(
    (item: InstalledItem, on: boolean, shift: boolean) => {
      setSelected((prev) => {
        const next = new Set(prev);
        if (shift && anchor) {
          const names = visible.map((i) => i.file_name);
          const [from, to] = [names.indexOf(anchor), names.indexOf(item.file_name)].sort((a, b) => a - b);
          if (from >= 0) for (const n of names.slice(from, to + 1)) next.add(n);
        } else if (on) next.add(item.file_name);
        else next.delete(item.file_name);
        return next;
      });
      setAnchor(item.file_name);
    },
    [anchor, visible],
  );

  const onToggle = useCallback(
    (item: InstalledItem, enabled: boolean) => {
      const requiredBy = deps?.requiredBy.get(item.file_name);
      if (!enabled && requiredBy && requiredBy.length > 0) setToDisable({ item, requiredBy });
      else setEnabled.mutate({ names: [item.file_name], enabled });
    },
    [deps, setEnabled],
  );

  const onDelete = useCallback((item: InstalledItem) => setToDelete([item]), []);

  const reveal = useCallback(
    (item: InstalledItem) => {
      const sep = instance.directory.includes("\\") ? "\\" : "/";
      const file = item.enabled || item.is_dir ? item.file_name : `${item.file_name}.disabled`;
      instancesApi
        .revealFile(instance.id, [instance.directory, meta.folder, file].join(sep))
        .catch((e) => notify.error({ title: "Dossier introuvable", message: errorMessage(e), history: false }));
    },
    [instance.id, instance.directory, meta.folder],
  );

  useEffect(() => {
    if (selected.size === 0) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") setSelected(new Set());
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [selected.size]);

  // Virtualised rows scroll with the page (the app's <main>).
  const scrollEl = useScrollContainer();
  const listRef = useRef<HTMLDivElement>(null);
  const [scrollMargin, setScrollMargin] = useState(0);
  const hasItems = items.length > 0;
  useLayoutEffect(() => {
    const measure = () => {
      if (!listRef.current || !scrollEl) return;
      const top = listRef.current.getBoundingClientRect().top - scrollEl.getBoundingClientRect().top;
      setScrollMargin(top + scrollEl.scrollTop);
    };
    measure();
    const observer = new ResizeObserver(measure);
    if (scrollEl) observer.observe(scrollEl);
    if (listRef.current?.parentElement) observer.observe(listRef.current.parentElement);
    return () => observer.disconnect();
  }, [scrollEl, problems, hasItems]);

  // TanStack Virtual returns fresh functions each render; the React Compiler
  // skipping memoisation of this component is the documented trade-off.
  // eslint-disable-next-line react-hooks/incompatible-library
  const virtualizer = useVirtualizer({
    count: visible.length,
    getScrollElement: () => scrollEl,
    estimateSize: () => ROW_HEIGHT,
    overscan: 10,
    scrollMargin,
  });

  const selectedItems = items.filter((i) => selected.has(i.file_name));
  const selecting = selected.size > 0;
  const filters: { value: Filter; label: string; count: number; hidden?: boolean }[] = [
    { value: "all", label: "Tous", count: items.length },
    { value: "enabled", label: "Actifs", count: enabledCount },
    { value: "disabled", label: "Désactivés", count: items.length - enabledCount },
    { value: "problems", label: "Problèmes", count: problems, hidden: problems === 0 },
  ];

  if (list.isLoading) {
    return (
      <div className="space-y-2">
        {[0, 1, 2, 3, 4, 5].map((i) => (
          <Skeleton key={i} className="h-16 rounded-xl" />
        ))}
      </div>
    );
  }
  if (list.isError) {
    return <p className="py-10 text-center text-sm text-destructive">{errorMessage(list.error)}</p>;
  }

  return (
    <>
      {items.length === 0 ? (
        <EmptyState
          icon={meta.icon}
          title={meta.emptyTitle}
          description={meta.emptyDescription}
          action={
            <Button onClick={() => onBrowse()} className="gap-1.5">
              <Compass aria-hidden="true" />
              Parcourir le catalogue
            </Button>
          }
        />
      ) : (
        <>
          {problems > 0 && filter !== "problems" && (
            <button
              type="button"
              onClick={() => setFilter("problems")}
              className="mb-3 flex w-full items-center gap-3 rounded-xl border border-destructive/30 bg-destructive/8 px-4 py-2.5 text-left transition-colors hover:bg-destructive/12"
            >
              <AlertTriangle className="size-4 shrink-0 text-destructive" aria-hidden="true" />
              <span className="flex-1 text-sm">
                <span className="font-semibold">
                  {problems} mod{problems > 1 ? "s ont" : " a"} un problème de dépendance ou d'incompatibilité
                </span>
                <span className="text-muted-foreground"> — le jeu risque de ne pas démarrer.</span>
              </span>
              <span className="text-xs font-medium text-destructive">Voir</span>
            </button>
          )}

          <div className="mb-3 flex flex-wrap items-center gap-2">
            <div className="glass flex rounded-lg p-0.5" role="tablist" aria-label="Filtrer">
              {filters
                .filter((f) => !f.hidden)
                .map((f) => (
                  <button
                    key={f.value}
                    role="tab"
                    aria-selected={filter === f.value}
                    onClick={() => setFilter(f.value)}
                    className={cn(
                      "rounded-md px-3 py-1 text-xs font-medium transition-colors",
                      filter === f.value
                        ? f.value === "problems"
                          ? "bg-destructive text-white"
                          : "bg-primary text-primary-foreground"
                        : "text-muted-foreground hover:text-foreground",
                    )}
                  >
                    {f.label} <span className="tabular-nums opacity-70">{f.count}</span>
                  </button>
                ))}
            </div>
            {identify.isFetching && (
              <span className="flex items-center gap-1.5 text-xs text-muted-foreground">
                <Loader2 className="size-3 animate-spin" aria-hidden="true" />
                Identification…
              </span>
            )}
            <Select value={sort} onValueChange={(v) => setSort(v as Sort)}>
              <SelectTrigger size="sm" className="ml-auto w-32" aria-label="Trier">
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
            <div className="relative">
              <Search
                className="pointer-events-none absolute top-1/2 left-2.5 size-3.5 -translate-y-1/2 text-muted-foreground"
                aria-hidden="true"
              />
              <Input
                type="search"
                placeholder={meta.searchPlaceholder}
                value={search}
                onChange={(e) => setSearch(e.target.value)}
                className="h-8 w-56 pl-8!"
              />
            </div>
          </div>

          <div className="glass overflow-hidden rounded-2xl">
            {visible.length === 0 ? (
              <p className="px-4 py-10 text-center text-sm text-muted-foreground">Rien ne correspond.</p>
            ) : (
              <div ref={listRef} className="relative" style={{ height: virtualizer.getTotalSize() }}>
                {virtualizer.getVirtualItems().map((row) => {
                  const item = visible[row.index];
                  return (
                    <div
                      key={item.file_name}
                      className="absolute inset-x-0 border-b border-border/50 last:border-b-0"
                      style={{ top: row.start - scrollMargin, height: ROW_HEIGHT }}
                    >
                      <InstalledRow
                        item={item}
                        kind={kind}
                        selected={selected.has(item.file_name)}
                        selecting={selecting}
                        missing={deps?.missing.get(item.file_name)}
                        conflicts={deps?.conflicts.get(item.file_name)}
                        requiredBy={deps?.requiredBy.get(item.file_name)}
                        busy={setEnabled.isPending}
                        onSelect={onSelect}
                        onToggle={onToggle}
                        onDelete={onDelete}
                        onReveal={() => reveal(item)}
                        onFindDependency={(id) => onBrowse(id)}
                      />
                    </div>
                  );
                })}
              </div>
            )}
          </div>
          {selecting && <div className="h-20" aria-hidden="true" />}
        </>
      )}

      <SelectionBar
        selected={selectedItems}
        busy={setEnabled.isPending}
        onSelectAll={() => setSelected(new Set(visible.map((i) => i.file_name)))}
        onSetEnabled={(names, enabled) => setEnabled.mutate({ names, enabled })}
        onDelete={setToDelete}
        onClear={() => setSelected(new Set())}
      />

      {dragging && (
        <div className="pointer-events-none fixed inset-4 z-50 flex items-center justify-center rounded-3xl border-2 border-dashed border-primary bg-background/80 backdrop-blur-sm">
          <p className="text-lg font-semibold text-primary">Dépose tes fichiers .{meta.extension} pour les ajouter</p>
        </div>
      )}

      <ConfirmDialog
        open={toDelete !== null}
        onOpenChange={(o) => !o && setToDelete(null)}
        title={toDelete && toDelete.length > 1 ? `Supprimer ${toDelete.length} éléments ?` : "Supprimer cet élément ?"}
        description={
          toDelete?.length === 1
            ? `« ${displayName(toDelete[0])} » sera supprimé du dossier ${meta.folder}.`
            : `Les fichiers sélectionnés seront supprimés du dossier ${meta.folder}.`
        }
        confirmLabel="Supprimer"
        destructive
        pending={remove.isPending}
        onConfirm={() => toDelete && remove.mutate(toDelete.map((i) => i.file_name))}
      />
      <ConfirmDialog
        open={toDisable !== null}
        onOpenChange={(o) => !o && setToDisable(null)}
        title={`Désactiver ${toDisable ? displayName(toDisable.item) : ""} ?`}
        description={
          toDisable
            ? `Il est requis par ${toDisable.requiredBy.slice(0, 4).join(", ")}${toDisable.requiredBy.length > 4 ? "…" : ""} : ces mods ne fonctionneront plus.`
            : ""
        }
        confirmLabel="Désactiver quand même"
        destructive
        onConfirm={() => {
          if (toDisable) setEnabled.mutate({ names: [toDisable.item.file_name], enabled: false });
          setToDisable(null);
        }}
      />
    </>
  );
}
