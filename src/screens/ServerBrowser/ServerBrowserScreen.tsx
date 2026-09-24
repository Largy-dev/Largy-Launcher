import { useMemo, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { useNavigate } from "react-router";
import { motion } from "motion/react";
import { ExternalLink, Loader2, Play, Plus, Search, Server, Settings2, Signal, Users, Wrench } from "lucide-react";
import { openUrl } from "@tauri-apps/plugin-opener";

import { EmptyState } from "@/components/EmptyState";
import { PageHeader } from "@/components/PageHeader";
import { Motd } from "@/components/servers/Motd";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { usePlayInstance } from "@/hooks/useLaunchInstance";
import { useServerStatus } from "@/hooks/useServerStatus";
import { listItem } from "@/lib/motion";
import { cn } from "@/lib/utils";
import { catalogApi, type FeaturedServer } from "@/services/servers";
import { instancesApi, type Instance } from "@/services/tauri";

import { PrepareServerDialog, type PrepareTarget } from "./PrepareServerDialog";

interface ServerCardProps {
  name: string;
  address: string;
  description?: string;
  tags?: string[];
  version: string;
  website?: string | null;
  index: number;
  /** The instance already prepared for this server, if any. */
  instance: Instance | undefined;
  onPrepare?: () => void;
}

function ServerCard({
  name,
  address,
  description,
  tags,
  version,
  website,
  index,
  instance,
  onPrepare,
}: ServerCardProps) {
  const navigate = useNavigate();
  const playInstance = usePlayInstance();
  const status = useServerStatus(address);

  return (
    <motion.article
      variants={listItem}
      initial="hidden"
      animate="show"
      custom={index}
      className="glass flex flex-col gap-3 rounded-2xl p-4"
    >
      <div className="flex items-start gap-3">
        {status.data?.favicon ? (
          <img src={status.data.favicon} alt="" className="size-14 shrink-0 rounded-xl [image-rendering:pixelated]" />
        ) : (
          <div className="flex size-14 shrink-0 items-center justify-center rounded-xl bg-muted">
            <Server className="size-6 text-muted-foreground" aria-hidden="true" />
          </div>
        )}
        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-1.5">
            <h3 className="truncate font-semibold">{name}</h3>
            {website && (
              <button
                onClick={() => openUrl(website)}
                title="Site du serveur"
                aria-label={`Site de ${name}`}
                className="text-muted-foreground transition-colors hover:text-foreground"
              >
                <ExternalLink className="size-3.5" aria-hidden="true" />
              </button>
            )}
          </div>
          <p className="truncate font-mono text-[0.7rem] text-muted-foreground">{address}</p>
          <div className="mt-1 flex items-center gap-3 text-xs">
            {status.isLoading ? (
              <span className="flex items-center gap-1 text-muted-foreground">
                <Loader2 className="size-3 animate-spin" aria-hidden="true" /> Connexion…
              </span>
            ) : status.data ? (
              <>
                <span className="flex items-center gap-1 text-success">
                  <Users className="size-3.5" aria-hidden="true" />
                  <span className="font-semibold tabular-nums">{status.data.online.toLocaleString("fr-FR")}</span>
                  en ligne
                </span>
                <span className="flex items-center gap-1 text-muted-foreground tabular-nums">
                  <Signal className="size-3.5" aria-hidden="true" />
                  {status.data.latency_ms} ms
                </span>
              </>
            ) : (
              <span className="text-destructive">Hors ligne</span>
            )}
          </div>
        </div>
      </div>

      {status.data?.motd && (
        <div className="rounded-lg bg-black/70 px-2.5 py-1.5">
          <Motd text={status.data.motd} />
        </div>
      )}
      {description && <p className="line-clamp-2 text-sm text-muted-foreground">{description}</p>}
      {tags && tags.length > 0 && (
        <div className="flex flex-wrap gap-1.5">
          {tags.map((tag) => (
            <span key={tag} className="rounded-full bg-accent px-2 py-0.5 text-[0.68rem] font-medium">
              {tag}
            </span>
          ))}
        </div>
      )}

      <div className="mt-auto flex items-center justify-between gap-2 pt-1">
        <span className="text-xs text-muted-foreground">Minecraft {version} · Fabric</span>
        {instance ? (
          <div className="flex gap-1.5">
            <Button
              variant="ghost"
              size="icon-sm"
              title="Réglages de l'instance"
              aria-label={`Réglages de ${instance.name}`}
              onClick={() => navigate(`/instances/${instance.id}`)}
            >
              <Settings2 aria-hidden="true" />
            </Button>
            <Button size="sm" className="bg-gradient-brand gap-1.5" onClick={() => playInstance(instance)}>
              <Play className="fill-current" aria-hidden="true" />
              Jouer
            </Button>
          </div>
        ) : (
          onPrepare && (
            <Button size="sm" variant="outline" className="gap-1.5" onClick={onPrepare}>
              <Wrench aria-hidden="true" />
              Préparer
            </Button>
          )
        )}
      </div>
    </motion.article>
  );
}

/** Popular servers, ready to play in one click, plus the ones the player added. */
export function ServerBrowserScreen() {
  const catalog = useQuery({ queryKey: ["server-catalog"], queryFn: catalogApi.load, staleTime: 30 * 60 * 1000 });
  const { data: instances } = useQuery({ queryKey: ["instances"], queryFn: instancesApi.list });
  const [query, setQuery] = useState("");
  const [tag, setTag] = useState<string | null>(null);
  const [target, setTarget] = useState<PrepareTarget | null>(null);

  const servers = useMemo(() => catalog.data?.servers ?? [], [catalog.data]);
  const tags = useMemo(() => {
    const counts = new Map<string, number>();
    servers.flatMap((s) => s.tags).forEach((t) => counts.set(t, (counts.get(t) ?? 0) + 1));
    return [...counts.entries()]
      .sort((a, b) => b[1] - a[1])
      .slice(0, 8)
      .map(([t]) => t);
  }, [servers]);

  const needle = query.trim().toLowerCase();
  const matches = (s: FeaturedServer) =>
    (!tag || s.tags.includes(tag)) &&
    (!needle || [s.name, s.address, s.description, ...s.tags].some((field) => field.toLowerCase().includes(needle)));
  const visible = servers.filter(matches);
  const preparedFor = (id: string) => instances?.find((i) => i.featured_server === id);
  const mine = (instances ?? []).filter((i) => i.featured_server === "custom" && i.auto_join_server);

  return (
    <div className="flex flex-1 flex-col">
      <PageHeader
        eyebrow="Multijoueur"
        title="Serveurs"
        description="Les grands serveurs, prêts à jouer : la bonne version et les mods utiles installés en un clic."
        action={
          <Button size="sm" className="gap-1.5" onClick={() => setTarget({ server: null })}>
            <Plus aria-hidden="true" />
            Ajouter mon serveur
          </Button>
        }
      />

      <div className="mb-4 flex flex-wrap items-center gap-2">
        <div className="relative w-64">
          <Search
            className="absolute top-1/2 left-2.5 size-4 -translate-y-1/2 text-muted-foreground"
            aria-hidden="true"
          />
          <Input
            className="pl-8"
            placeholder="Rechercher un serveur…"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
          />
        </div>
        {tags.map((t) => (
          <button
            key={t}
            onClick={() => setTag(tag === t ? null : t)}
            aria-pressed={tag === t}
            className={cn(
              "rounded-full px-3 py-1 text-xs font-medium transition-colors",
              tag === t ? "bg-primary text-primary-foreground" : "bg-accent/60 hover:bg-accent",
            )}
          >
            {t}
          </button>
        ))}
      </div>

      {mine.length > 0 && (
        <section className="mb-6">
          <h3 className="mb-2 px-1 text-[0.7rem] font-semibold tracking-wider text-muted-foreground uppercase">
            Mes serveurs
          </h3>
          <div className="grid grid-cols-1 gap-4 lg:grid-cols-2 xl:grid-cols-3">
            {mine.map((instance, index) => (
              <ServerCard
                key={instance.id}
                name={instance.name}
                address={instance.auto_join_server!}
                version={instance.minecraft_version}
                index={index}
                instance={instance}
              />
            ))}
          </div>
        </section>
      )}

      {catalog.isLoading ? (
        <div className="flex flex-1 items-center justify-center">
          <Loader2 className="size-5 animate-spin text-muted-foreground" aria-hidden="true" />
        </div>
      ) : visible.length === 0 ? (
        <EmptyState
          icon={Server}
          title="Aucun serveur trouvé"
          description="Essaie un autre mot-clé, ou ajoute ton propre serveur."
        />
      ) : (
        <section>
          <h3 className="mb-2 px-1 text-[0.7rem] font-semibold tracking-wider text-muted-foreground uppercase">
            Serveurs populaires
          </h3>
          <div className="grid grid-cols-1 gap-4 pb-8 lg:grid-cols-2 xl:grid-cols-3">
            {visible.map((server, index) => (
              <ServerCard
                key={server.id}
                name={server.name}
                address={server.address}
                description={server.description}
                tags={server.tags}
                version={server.minecraft_version}
                website={server.website}
                index={index}
                instance={preparedFor(server.id)}
                onPrepare={() => setTarget({ server })}
              />
            ))}
          </div>
        </section>
      )}

      <PrepareServerDialog
        target={target}
        presets={catalog.data?.presets ?? []}
        onOpenChange={(open) => !open && setTarget(null)}
      />
    </div>
  );
}
