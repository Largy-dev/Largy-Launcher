import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { motion } from "motion/react";
import { Download, PackageSearch, Search } from "lucide-react";

import { EmptyState } from "@/components/EmptyState";
import { PageHeader } from "@/components/PageHeader";
import { Skeleton } from "@/components/Skeleton";
import { Input } from "@/components/ui/input";
import { useSettings } from "@/hooks/useSettings";
import { fadeUp, stagger } from "@/lib/motion";
import { cn } from "@/lib/utils";
import { errorMessage, providersApi, type ModpackSummary, type ProviderId } from "@/services/tauri";

import { ModpackDetailDialog } from "./ModpackDetailDialog";

const PROVIDERS: { id: ProviderId; label: string; color: string }[] = [
  { id: "ftb", label: "FTB", color: "#e5484d" },
  { id: "curseforge", label: "CurseForge", color: "#f16436" },
];

function ModpackCard({ pack, onSelect }: { pack: ModpackSummary; onSelect: () => void }) {
  return (
    <motion.button
      variants={fadeUp}
      whileHover={{ y: -4 }}
      onClick={onSelect}
      className="glass group relative flex flex-col overflow-hidden rounded-2xl text-left transition-shadow hover:shadow-xl"
    >
      <div className="relative h-28 overflow-hidden">
        {pack.icon_url ? (
          <img
            src={pack.icon_url}
            alt=""
            aria-hidden="true"
            className="absolute inset-0 size-full scale-125 object-cover opacity-60 blur-md transition-transform duration-500 group-hover:scale-150"
          />
        ) : (
          <div className="bg-gradient-brand absolute inset-0 opacity-50" />
        )}
        <div className="absolute inset-0 bg-gradient-to-b from-transparent to-card/90" />
        <span className="bg-gradient-brand absolute top-2.5 right-2.5 flex items-center gap-1 rounded-full px-2.5 py-1 text-[0.68rem] font-bold text-primary-foreground opacity-0 shadow-glow transition-opacity group-hover:opacity-100">
          <Download className="size-3" aria-hidden="true" />
          Installer
        </span>
      </div>
      <div className="relative -mt-10 flex flex-1 flex-col gap-2 px-4 pb-4">
        {pack.icon_url ? (
          <img src={pack.icon_url} alt="" className="size-16 rounded-xl object-cover shadow-lg ring-4 ring-card" />
        ) : (
          <div className="flex size-16 items-center justify-center rounded-xl bg-muted ring-4 ring-card">
            <PackageSearch className="size-6 text-muted-foreground" aria-hidden="true" />
          </div>
        )}
        <div className="min-w-0">
          <h3 className="truncate font-bold">{pack.name}</h3>
          {pack.author && <p className="truncate text-xs text-muted-foreground">par {pack.author}</p>}
        </div>
        <p className="line-clamp-2 text-xs text-muted-foreground">{pack.summary}</p>
      </div>
    </motion.button>
  );
}

function ModpackGrid({ provider, onSelect }: { provider: ProviderId; onSelect: (pack: ModpackSummary) => void }) {
  const [text, setText] = useState("");
  const [query, setQuery] = useState("");

  const { data, isLoading, isError, error } = useQuery({
    queryKey: ["modpack-search", provider, query],
    queryFn: () => providersApi.search(provider, query),
  });

  return (
    <div className="flex flex-1 flex-col">
      <form
        className="relative mb-5"
        onSubmit={(e) => {
          e.preventDefault();
          setQuery(text);
        }}
      >
        <Search
          className="pointer-events-none absolute top-1/2 left-4 size-4 -translate-y-1/2 text-muted-foreground"
          aria-hidden="true"
        />
        <Input
          type="search"
          placeholder="Rechercher un modpack… (Entrée pour lancer la recherche)"
          className="glass h-11 rounded-xl pl-11! text-base"
          value={text}
          onChange={(e) => setText(e.target.value)}
        />
      </form>

      {isLoading ? (
        <div className="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-3 2xl:grid-cols-4">
          {Array.from({ length: 6 }, (_, i) => (
            <Skeleton key={i} className="h-52 rounded-2xl" />
          ))}
        </div>
      ) : isError ? (
        <EmptyState icon={PackageSearch} title="Erreur" description={errorMessage(error)} />
      ) : !data || data.length === 0 ? (
        <EmptyState
          icon={PackageSearch}
          title="Aucun modpack trouvé"
          description="Essaie un autre terme de recherche."
        />
      ) : (
        <motion.div
          key={query}
          variants={stagger}
          initial="hidden"
          animate="show"
          className="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-3 2xl:grid-cols-4"
        >
          {data.map((pack) => (
            <ModpackCard key={pack.id} pack={pack} onSelect={() => onSelect(pack)} />
          ))}
        </motion.div>
      )}
    </div>
  );
}

export function ModpackBrowserScreen() {
  const [provider, setProvider] = useState<ProviderId>("ftb");
  const [selected, setSelected] = useState<ModpackSummary | null>(null);
  const { data: settings } = useSettings();
  const curseforgeEnabled = !!settings?.curseforge_api_key.trim();
  const providers = curseforgeEnabled ? PROVIDERS : PROVIDERS.filter((p) => p.id === "ftb");
  const active = curseforgeEnabled ? provider : "ftb";

  return (
    <div className="flex flex-1 flex-col">
      <PageHeader
        eyebrow="Découvrir"
        title="Modpacks"
        description={
          curseforgeEnabled
            ? "Des centaines d'aventures prêtes à jouer, installées en un clic."
            : "Des modpacks FTB prêts à jouer. Ajoute une clé CurseForge dans Paramètres › Avancé pour en débloquer plus."
        }
        action={
          providers.length > 1 && (
            <div className="glass flex rounded-xl p-1">
              {providers.map((p) => (
                <button
                  key={p.id}
                  onClick={() => setProvider(p.id)}
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
          )
        }
      />

      <ModpackGrid key={active} provider={active} onSelect={setSelected} />

      <ModpackDetailDialog provider={active} pack={selected} onOpenChange={(open) => !open && setSelected(null)} />
    </div>
  );
}
