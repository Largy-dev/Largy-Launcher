import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Check, Download, Loader2, Search } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { formatCount } from "@/lib/format";
import { notify } from "@/lib/notify";
import { cn } from "@/lib/utils";
import { contentApi, errorMessage, type ContentHit, type ContentKind, type Instance } from "@/services/tauri";

const KINDS: { id: ContentKind; label: string; modsOnly?: boolean }[] = [
  { id: "mod", label: "Mods", modsOnly: true },
  { id: "resource_pack", label: "Resource packs" },
  { id: "shader", label: "Shaders" },
];

interface ContentBrowserDialogProps {
  instance: Instance;
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

/** Modrinth catalogue filtered to what's compatible with this instance. */
export function ContentBrowserDialog({ instance, open, onOpenChange }: ContentBrowserDialogProps) {
  const queryClient = useQueryClient();
  const kinds = KINDS.filter((k) => !k.modsOnly || instance.loader !== "vanilla");
  const [kind, setKind] = useState<ContentKind>(kinds[0].id);
  const [text, setText] = useState("");
  const [query, setQuery] = useState("");
  const [installed, setInstalled] = useState<Set<string>>(new Set());

  const results = useQuery({
    queryKey: ["content-search", instance.id, kind, query],
    queryFn: () => contentApi.search(instance.id, kind, query),
    enabled: open,
    staleTime: 60_000,
  });

  const install = useMutation({
    mutationFn: (hit: ContentHit) => contentApi.install(instance.id, hit.project_id, kind),
    onSuccess: (files, hit) => {
      setInstalled((prev) => new Set(prev).add(hit.project_id));
      queryClient.invalidateQueries({ queryKey: ["instance-mods", instance.id] });
      const extra = files.length > 1 ? ` (+ ${files.length - 1} dépendance${files.length > 2 ? "s" : ""})` : "";
      notify.success({ title: `${hit.title} installé${extra}`, history: false });
    },
    onError: (e, hit) => notify.error({ title: `Impossible d'installer ${hit.title}`, message: errorMessage(e) }),
  });

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="flex max-h-[85vh] flex-col sm:max-w-3xl">
        <DialogHeader>
          <DialogTitle>Parcourir Modrinth</DialogTitle>
          <DialogDescription>
            Compatible avec Minecraft {instance.minecraft_version}
            {instance.loader !== "vanilla" && ` · ${instance.loader}`}. Les dépendances requises sont installées
            automatiquement.
          </DialogDescription>
        </DialogHeader>

        <div className="flex flex-wrap items-center gap-2">
          <div className="glass flex rounded-lg p-0.5">
            {kinds.map((k) => (
              <button
                key={k.id}
                onClick={() => setKind(k.id)}
                className={cn(
                  "rounded-md px-3 py-1 text-xs font-medium transition-colors",
                  kind === k.id ? "bg-primary text-primary-foreground" : "text-muted-foreground hover:text-foreground",
                )}
              >
                {k.label}
              </button>
            ))}
          </div>
          <form
            className="relative ml-auto"
            onSubmit={(e) => {
              e.preventDefault();
              setQuery(text.trim());
            }}
          >
            <Search
              className="pointer-events-none absolute top-1/2 left-2.5 size-3.5 -translate-y-1/2 text-muted-foreground"
              aria-hidden="true"
            />
            <Input
              type="search"
              placeholder="Rechercher puis Entrée…"
              value={text}
              onChange={(e) => {
                setText(e.target.value);
                if (!e.target.value) setQuery("");
              }}
              className="h-8 w-64 pl-8!"
            />
          </form>
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
          {results.data?.length === 0 && (
            <p className="py-8 text-center text-sm text-muted-foreground">Aucun résultat compatible.</p>
          )}
          <ul className="divide-y divide-border/60">
            {results.data?.map((hit) => {
              const done = installed.has(hit.project_id);
              const pending = install.isPending && install.variables?.project_id === hit.project_id;
              return (
                <li key={hit.project_id} className="flex items-center gap-3 py-2.5">
                  {hit.icon_url ? (
                    <img src={hit.icon_url} alt="" className="size-10 shrink-0 rounded-lg object-cover" />
                  ) : (
                    <div className="size-10 shrink-0 rounded-lg bg-muted" />
                  )}
                  <div className="min-w-0 flex-1">
                    <p className="truncate text-sm font-medium">
                      {hit.title}
                      <span className="ml-2 text-xs font-normal text-muted-foreground">par {hit.author}</span>
                    </p>
                    <p className="line-clamp-1 text-xs text-muted-foreground">{hit.description}</p>
                  </div>
                  <span className="hidden shrink-0 text-xs text-muted-foreground tabular-nums sm:block">
                    {formatCount(hit.downloads)}
                  </span>
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
        </div>
      </DialogContent>
    </Dialog>
  );
}
