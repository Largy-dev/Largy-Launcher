import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { Loader2, PackageSearch, Search } from "lucide-react";

import { EmptyState } from "@/components/EmptyState";
import { PageHeader } from "@/components/PageHeader";
import { Card, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { useSettings } from "@/hooks/useSettings";
import { errorMessage, providersApi, type ModpackSummary, type ProviderId } from "@/services/tauri";

import { ModpackDetailDialog } from "./ModpackDetailDialog";

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
        className="relative mb-4"
        onSubmit={(e) => {
          e.preventDefault();
          setQuery(text);
        }}
      >
        <Search
          className="pointer-events-none absolute left-3 top-1/2 size-4 -translate-y-1/2 text-muted-foreground"
          aria-hidden="true"
        />
        <Input
          type="search"
          placeholder="Rechercher un modpack…"
          className="pl-9"
          value={text}
          onChange={(e) => setText(e.target.value)}
        />
      </form>

      {isLoading ? (
        <div className="flex flex-1 items-center justify-center gap-2 text-sm text-muted-foreground">
          <Loader2 className="size-4 animate-spin" aria-hidden="true" />
          Chargement…
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
        <div className="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-3">
          {data.map((pack) => (
            <Card
              key={pack.id}
              onClick={() => onSelect(pack)}
              className="cursor-pointer transition-colors hover:bg-muted/50"
            >
              <CardHeader className="flex-row items-center gap-3 space-y-0">
                {pack.icon_url ? (
                  <img src={pack.icon_url} alt="" className="size-10 rounded-md object-cover" />
                ) : (
                  <div className="flex size-10 shrink-0 items-center justify-center rounded-md bg-muted">
                    <PackageSearch className="size-5 text-muted-foreground" aria-hidden="true" />
                  </div>
                )}
                <div className="min-w-0 flex-1">
                  <CardTitle className="truncate">{pack.name}</CardTitle>
                  <CardDescription className="line-clamp-2">{pack.summary || pack.author}</CardDescription>
                </div>
              </CardHeader>
            </Card>
          ))}
        </div>
      )}
    </div>
  );
}

export function ModpackBrowserScreen() {
  const [provider, setProvider] = useState<ProviderId>("ftb");
  const [selected, setSelected] = useState<ModpackSummary | null>(null);
  const { data: settings } = useSettings();
  const curseforgeEnabled = !!settings?.curseforge_api_key.trim();

  return (
    <div className="flex flex-1 flex-col">
      <PageHeader
        title="Modpacks"
        description={
          curseforgeEnabled
            ? "Parcours et installe des modpacks FTB ou CurseForge."
            : "Parcours et installe des modpacks FTB."
        }
      />

      {curseforgeEnabled ? (
        <Tabs value={provider} onValueChange={(v) => setProvider(v as ProviderId)} className="flex flex-1 flex-col">
          <TabsList>
            <TabsTrigger value="ftb">FTB</TabsTrigger>
            <TabsTrigger value="curseforge">CurseForge</TabsTrigger>
          </TabsList>
          <TabsContent value="ftb" className="flex flex-1 flex-col">
            <ModpackGrid provider="ftb" onSelect={setSelected} />
          </TabsContent>
          <TabsContent value="curseforge" className="flex flex-1 flex-col">
            <ModpackGrid provider="curseforge" onSelect={setSelected} />
          </TabsContent>
        </Tabs>
      ) : (
        <ModpackGrid provider="ftb" onSelect={setSelected} />
      )}

      <ModpackDetailDialog
        provider={provider}
        pack={selected}
        onOpenChange={(open) => !open && setSelected(null)}
      />
    </div>
  );
}
