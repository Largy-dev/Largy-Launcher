import { useEffect, useMemo, useState, type ReactNode } from "react";
import { useQuery } from "@tanstack/react-query";
import { useNavigate } from "react-router";
import { Compass, LayoutGrid, Search, Server, Settings2 } from "lucide-react";

import { InstanceIcon } from "@/components/instance/InstanceIcon";
import { Dialog, DialogContent, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { instancesApi } from "@/services/tauri";

interface Entry {
  key: string;
  label: string;
  hint?: string;
  icon: ReactNode;
  go: () => void;
}

const STATIC_ENTRIES: Omit<Entry, "go">[] = [
  { key: "home", label: "Instances", icon: <LayoutGrid className="size-4" aria-hidden="true" /> },
  { key: "modpacks", label: "Modpacks", icon: <Compass className="size-4" aria-hidden="true" /> },
  { key: "servers", label: "Serveurs", icon: <Server className="size-4" aria-hidden="true" /> },
  { key: "settings", label: "Paramètres", icon: <Settings2 className="size-4" aria-hidden="true" /> },
];

const ROUTES: Record<string, string> = { home: "/", modpacks: "/modpacks", servers: "/servers", settings: "/settings" };

/** Ctrl/Cmd+K quick switcher: jump to any instance or the main sections. */
export function CommandPalette() {
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  const navigate = useNavigate();
  const { data: instances } = useQuery({ queryKey: ["instances"], queryFn: instancesApi.list, enabled: open });

  useEffect(() => {
    function onKeyDown(e: KeyboardEvent) {
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "k") {
        e.preventDefault();
        setQuery("");
        setOpen((o) => !o);
      }
    }
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, []);

  function setOpenAndResetQuery(next: boolean) {
    setOpen(next);
    setQuery("");
  }

  const entries: Entry[] = useMemo(() => {
    const nav: Entry[] = STATIC_ENTRIES.map((e) => ({ ...e, go: () => navigate(ROUTES[e.key]) }));
    const fromInstances: Entry[] = (instances ?? []).map((i) => ({
      key: i.id,
      label: i.name,
      hint: `${i.minecraft_version} · ${i.loader}`,
      icon: <InstanceIcon instance={i} className="size-6 rounded-md" />,
      go: () => navigate(`/instances/${i.id}`),
    }));
    return [...fromInstances, ...nav];
  }, [instances, navigate]);

  const needle = query.trim().toLowerCase();
  const visible = needle
    ? entries.filter((e) => e.label.toLowerCase().includes(needle) || e.hint?.toLowerCase().includes(needle))
    : entries;

  function select(entry: Entry) {
    entry.go();
    setOpenAndResetQuery(false);
  }

  return (
    <Dialog open={open} onOpenChange={setOpenAndResetQuery}>
      <DialogContent className="gap-3 sm:max-w-md" showCloseButton={false}>
        <DialogHeader>
          <DialogTitle className="sr-only">Recherche rapide</DialogTitle>
          <div className="relative">
            <Search
              className="pointer-events-none absolute top-1/2 left-3 size-4 -translate-y-1/2 text-muted-foreground"
              aria-hidden="true"
            />
            <Input
              autoFocus
              placeholder="Rechercher une instance, une page…"
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && visible[0] && select(visible[0])}
              className="pl-9!"
            />
          </div>
        </DialogHeader>
        <div className="-mx-1 max-h-80 overflow-y-auto">
          {visible.length === 0 ? (
            <p className="py-6 text-center text-sm text-muted-foreground">Aucun résultat.</p>
          ) : (
            visible.map((entry) => (
              <button
                key={entry.key}
                onClick={() => select(entry)}
                className="flex w-full items-center gap-3 rounded-lg px-3 py-2 text-left text-sm hover:bg-accent"
              >
                {entry.icon}
                <span className="min-w-0 flex-1 truncate">{entry.label}</span>
                {entry.hint && <span className="shrink-0 text-xs text-muted-foreground">{entry.hint}</span>}
              </button>
            ))
          )}
        </div>
        <p className="text-center text-[0.65rem] text-muted-foreground">Ctrl+K pour rouvrir · Échap pour fermer</p>
      </DialogContent>
    </Dialog>
  );
}
