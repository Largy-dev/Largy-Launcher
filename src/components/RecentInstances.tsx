import { useQuery } from "@tanstack/react-query";
import { useNavigate } from "react-router";
import { Blocks, History } from "lucide-react";

import { MinecraftGrassIcon } from "@/components/MinecraftGrassIcon";
import { instancesApi } from "@/services/tauri";
import { useAppStore } from "@/store/appStore";

export function RecentInstances() {
  const navigate = useNavigate();
  const runtime = useAppStore((s) => s.runtime);
  const { data: instances } = useQuery({ queryKey: ["instances"], queryFn: instancesApi.list });

  const recent = [...(instances ?? [])]
    .sort((a, b) => (b.last_played_at ?? b.created_at) - (a.last_played_at ?? a.created_at))
    .slice(0, 5);

  if (recent.length === 0) return null;

  return (
    <div className="mt-4 flex flex-col gap-0.5 px-2">
      <p className="flex items-center gap-1.5 px-2.5 py-1 text-xs font-medium tracking-wide text-sidebar-foreground/40 uppercase">
        <History className="size-3" aria-hidden="true" />
        Récentes
      </p>
      {recent.map((instance) => {
        const running = runtime[instance.id]?.running ?? false;
        return (
          <button
            key={instance.id}
            onClick={() => navigate(running ? `/instances/${instance.id}/launch` : `/instances/${instance.id}`)}
            className="flex items-center gap-2 rounded-md px-2.5 py-1.5 text-left text-sm text-sidebar-foreground/65 transition-colors hover:bg-sidebar-accent/60 hover:text-sidebar-foreground"
          >
            {instance.icon_url ? (
              <img src={instance.icon_url} alt="" className="size-5 shrink-0 rounded object-cover" />
            ) : instance.loader === "vanilla" ? (
              <MinecraftGrassIcon className="size-5 shrink-0 rounded" />
            ) : (
              <Blocks className="size-4 shrink-0 text-sidebar-foreground/40" aria-hidden="true" />
            )}
            <span className="min-w-0 flex-1 truncate">{instance.name}</span>
            {running && <span className="size-1.5 shrink-0 rounded-full bg-primary" aria-hidden="true" />}
          </button>
        );
      })}
    </div>
  );
}
