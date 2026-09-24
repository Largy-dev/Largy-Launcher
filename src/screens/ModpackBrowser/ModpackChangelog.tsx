import { useQuery } from "@tanstack/react-query";
import { Loader2, ScrollText } from "lucide-react";

import { changelogText } from "@/lib/changelog";
import { providersApi, type ProviderId } from "@/services/tauri";

interface ModpackChangelogProps {
  provider: ProviderId;
  packId: string;
  versionId: string;
}

/** Release notes of the selected version, fetched on demand. */
export function ModpackChangelog({ provider, packId, versionId }: ModpackChangelogProps) {
  const { data, isLoading, isError } = useQuery({
    queryKey: ["modpack-changelog", provider, packId, versionId],
    queryFn: () => providersApi.getChangelog(provider, packId, versionId),
    staleTime: 30 * 60 * 1000,
  });

  return (
    <div className="space-y-1.5">
      <p className="flex items-center gap-1.5 text-sm font-medium">
        <ScrollText className="size-3.5 text-muted-foreground" aria-hidden="true" />
        Nouveautés de cette version
      </p>
      <div className="max-h-44 overflow-y-auto rounded-lg border border-border/60 bg-muted/30 px-3 py-2 text-xs leading-relaxed whitespace-pre-wrap text-muted-foreground">
        {isLoading ? (
          <span className="flex items-center gap-2">
            <Loader2 className="size-3.5 animate-spin" aria-hidden="true" /> Chargement…
          </span>
        ) : isError ? (
          "Notes de version indisponibles pour le moment."
        ) : data ? (
          changelogText(data)
        ) : (
          "L'auteur n'a pas publié de notes pour cette version."
        )}
      </div>
    </div>
  );
}
