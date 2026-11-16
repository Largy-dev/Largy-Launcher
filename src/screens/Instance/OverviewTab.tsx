import type { ReactNode } from "react";
import { useQuery } from "@tanstack/react-query";
import { convertFileSrc } from "@tauri-apps/api/core";
import { ChevronRight, Globe, Images, NotebookPen, Server, Users } from "lucide-react";

import { Button } from "@/components/ui/button";
import { usePlayInstance } from "@/hooks/useLaunchInstance";
import { useServerStatus } from "@/hooks/useServerStatus";
import { formatDuration, formatRelative } from "@/lib/format";
import { worldsApi, type World } from "@/services/content";
import { serversApi, type ServerEntry } from "@/services/servers";
import { instancesApi, type Instance } from "@/services/tauri";

import type { InstanceTab } from "./tabs";

const GAME_MODES: Record<string, string> = {
  survival: "Survie",
  creative: "Créatif",
  adventure: "Aventure",
  spectator: "Spectateur",
};

function Card({
  title,
  action,
  children,
  className,
}: {
  title: string;
  action?: ReactNode;
  children: ReactNode;
  className?: string;
}) {
  return (
    <section className={className}>
      <div className="mb-2 flex items-center justify-between px-1">
        <h3 className="text-xs font-semibold tracking-wider text-muted-foreground uppercase">{title}</h3>
        {action}
      </div>
      {children}
    </section>
  );
}

function SeeAll({ onClick, label = "Tout voir" }: { onClick: () => void; label?: string }) {
  return (
    <button
      onClick={onClick}
      className="flex items-center gap-0.5 text-xs font-medium text-muted-foreground transition-colors hover:text-foreground"
    >
      {label}
      <ChevronRight className="size-3.5" aria-hidden="true" />
    </button>
  );
}

function Stat({ label, value }: { label: string; value: string }) {
  return (
    <div className="rounded-xl bg-foreground/4 px-4 py-3">
      <p className="truncate text-lg font-bold tabular-nums">{value}</p>
      <p className="text-xs text-muted-foreground">{label}</p>
    </div>
  );
}

/** The last sessions as bars, oldest on the left — the shape of how this instance gets played. */
function SessionsChart({ instance }: { instance: Instance }) {
  const sessions = [...instance.sessions].reverse();
  if (sessions.length < 2) return null;
  const longest = Math.max(...sessions.map((s) => s.duration_seconds), 1);
  const average = sessions.reduce((n, s) => n + s.duration_seconds, 0) / sessions.length;

  return (
    <Card
      title="Dernières sessions"
      action={<span className="text-xs text-muted-foreground">Moyenne : {formatDuration(average)}</span>}
    >
      <div className="flex h-24 items-end gap-1.5 rounded-xl bg-foreground/4 px-4 pt-4 pb-3">
        {sessions.map((session) => (
          <div
            key={session.started_at}
            title={`${formatRelative(session.started_at)} · ${formatDuration(session.duration_seconds)}`}
            className="max-w-10 min-w-1 flex-1 rounded-t-[3px] bg-primary/70 transition-colors hover:bg-primary"
            style={{ height: `${Math.max(6, (session.duration_seconds / longest) * 100)}%` }}
          />
        ))}
      </div>
    </Card>
  );
}

function WorldRow({ world, onOpen }: { world: World; onOpen: () => void }) {
  const details = [
    world.hardcore ? "Hardcore" : world.game_mode ? (GAME_MODES[world.game_mode] ?? world.game_mode) : null,
    world.last_played ? formatRelative(world.last_played) : null,
  ].filter(Boolean);
  return (
    <button
      onClick={onOpen}
      className="flex w-full items-center gap-3 rounded-lg px-3 py-2 text-left transition-colors hover:bg-foreground/5"
    >
      {world.icon_path ? (
        <img
          src={convertFileSrc(world.icon_path)}
          alt=""
          className="size-9 shrink-0 rounded-md [image-rendering:pixelated]"
        />
      ) : (
        <span className="flex size-9 shrink-0 items-center justify-center rounded-md bg-foreground/8">
          <Globe className="size-4 text-muted-foreground" aria-hidden="true" />
        </span>
      )}
      <span className="min-w-0 flex-1">
        <span className="block truncate text-sm font-medium">{world.name}</span>
        <span className="block truncate text-xs text-muted-foreground">{details.join(" · ")}</span>
      </span>
    </button>
  );
}

function ServerRow({ server, onJoin }: { server: ServerEntry; onJoin: () => void }) {
  const status = useServerStatus(server.address);
  const online = !!status.data;
  return (
    <div className="flex items-center gap-3 rounded-lg px-3 py-2 transition-colors hover:bg-foreground/5">
      <span
        className={
          status.isLoading
            ? "size-2 shrink-0 animate-pulse rounded-full bg-muted-foreground"
            : online
              ? "size-2 shrink-0 rounded-full bg-success shadow-[0_0_8px] shadow-success/60"
              : "size-2 shrink-0 rounded-full bg-destructive/70"
        }
        aria-label={online ? "En ligne" : "Hors ligne"}
      />
      <span className="min-w-0 flex-1">
        <span className="block truncate text-sm font-medium">{server.name || server.address}</span>
        <span className="flex items-center gap-1 truncate text-xs text-muted-foreground">
          {server.address}
          {status.data && (
            <>
              <span>·</span>
              <Users className="size-3" aria-hidden="true" />
              {status.data.online.toLocaleString("fr-FR")} sur {status.data.max.toLocaleString("fr-FR")}
            </>
          )}
        </span>
      </span>
      <Button size="sm" variant="secondary" onClick={onJoin}>
        Rejoindre
      </Button>
    </div>
  );
}

function EmptyLine({ icon: Icon, children }: { icon: typeof Globe; children: ReactNode }) {
  return (
    <p className="flex items-center gap-2 rounded-xl bg-foreground/4 px-4 py-5 text-sm text-muted-foreground">
      <Icon className="size-4 shrink-0" aria-hidden="true" />
      {children}
    </p>
  );
}

interface OverviewTabProps {
  instance: Instance;
  onNavigate: (tab: InstanceTab) => void;
}

/** One glance at the instance: numbers, recent worlds, servers and screenshots. */
export function OverviewTab({ instance, onNavigate }: OverviewTabProps) {
  const playInstance = usePlayInstance();
  const { data: worlds } = useQuery({
    queryKey: ["worlds", instance.id],
    queryFn: () => worldsApi.list(instance.id),
  });
  const { data: servers } = useQuery({
    queryKey: ["instance-servers", instance.id],
    queryFn: () => serversApi.list(instance.id),
  });
  const { data: screenshots } = useQuery({
    queryKey: ["instance-screenshots", instance.id],
    queryFn: () => instancesApi.screenshots(instance.id),
  });

  const recentWorlds = [...(worlds ?? [])].sort((a, b) => (b.last_played ?? 0) - (a.last_played ?? 0)).slice(0, 4);
  const sessions = instance.sessions.length;

  return (
    <div className="space-y-6">
      <div className="grid grid-cols-2 gap-2 lg:grid-cols-4">
        <Stat
          label="Temps de jeu"
          value={instance.play_time_seconds > 0 ? formatDuration(instance.play_time_seconds) : "—"}
        />
        <Stat
          label="Dernière partie"
          value={instance.last_played_at ? formatRelative(instance.last_played_at) : "Jamais"}
        />
        <Stat
          label="Session moyenne"
          value={
            sessions > 0
              ? formatDuration(instance.sessions.reduce((n, s) => n + s.duration_seconds, 0) / sessions)
              : "—"
          }
        />
        <Stat label="Créée" value={formatRelative(instance.created_at)} />
      </div>

      <SessionsChart instance={instance} />

      <div className="grid gap-6 lg:grid-cols-2">
        <Card title="Mondes récents" action={worlds?.length ? <SeeAll onClick={() => onNavigate("worlds")} /> : null}>
          {recentWorlds.length === 0 ? (
            <EmptyLine icon={Globe}>Aucun monde pour l'instant — lance le jeu pour en créer un.</EmptyLine>
          ) : (
            <div className="-mx-1">
              {recentWorlds.map((world) => (
                <WorldRow key={world.folder} world={world} onOpen={() => onNavigate("worlds")} />
              ))}
            </div>
          )}
        </Card>
        <Card title="Serveurs" action={<SeeAll onClick={() => onNavigate("servers")} label="Gérer" />}>
          {!servers?.length ? (
            <EmptyLine icon={Server}>
              Aucun serveur enregistré. Colle une adresse dans le champ « Rejoindre » ci-dessus.
            </EmptyLine>
          ) : (
            <div className="-mx-1">
              {servers.slice(0, 4).map((server, i) => (
                <ServerRow
                  key={`${i}-${server.address}`}
                  server={server}
                  onJoin={() => playInstance(instance, server.address)}
                />
              ))}
            </div>
          )}
        </Card>
      </div>

      {!!screenshots?.length && (
        <Card title="Captures récentes" action={<SeeAll onClick={() => onNavigate("screenshots")} />}>
          <div className="grid grid-cols-3 gap-2 xl:grid-cols-6">
            {screenshots.slice(0, 6).map((shot) => (
              <button
                key={shot.file_name}
                onClick={() => onNavigate("screenshots")}
                className="group aspect-video overflow-hidden rounded-lg bg-foreground/5"
              >
                <img
                  src={convertFileSrc(shot.path)}
                  alt=""
                  loading="lazy"
                  className="size-full object-cover transition-transform duration-300 group-hover:scale-105"
                />
              </button>
            ))}
          </div>
        </Card>
      )}
      {!screenshots?.length && (
        <Card title="Captures">
          <EmptyLine icon={Images}>Appuie sur F2 en jeu : tes captures apparaîtront ici.</EmptyLine>
        </Card>
      )}

      {instance.notes.trim() && (
        <Card title="Notes" action={<SeeAll onClick={() => onNavigate("settings")} label="Modifier" />}>
          <p className="flex gap-2 rounded-xl bg-foreground/4 px-4 py-3 text-sm whitespace-pre-wrap">
            <NotebookPen className="mt-0.5 size-4 shrink-0 text-muted-foreground" aria-hidden="true" />
            {instance.notes}
          </p>
        </Card>
      )}
    </div>
  );
}
