import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Loader2, Pencil, Play, Plus, RefreshCw, Server, Signal, Trash2, Users } from "lucide-react";

import { ConfirmDialog } from "@/components/ConfirmDialog";
import { EmptyState } from "@/components/EmptyState";
import { Motd } from "@/components/servers/Motd";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { usePlayInstance } from "@/hooks/useLaunchInstance";
import { motdPlainText } from "@/lib/motd";
import { notify } from "@/lib/notify";
import { cn } from "@/lib/utils";
import { serversApi, type ServerEntry } from "@/services/servers";
import { errorMessage, type Instance } from "@/services/tauri";

function latencyColor(ms: number) {
  if (ms < 80) return "text-success";
  if (ms < 200) return "text-warning";
  return "text-destructive";
}

function ServerRow({
  server,
  onPlay,
  onEdit,
  onDelete,
}: {
  server: ServerEntry;
  onPlay: () => void;
  onEdit: () => void;
  onDelete: () => void;
}) {
  const status = useQuery({
    queryKey: ["server-ping", server.address],
    queryFn: () => serversApi.ping(server.address),
    staleTime: 60 * 1000,
    retry: false,
  });
  const icon = status.data?.favicon ?? (server.icon ? `data:image/png;base64,${server.icon}` : null);

  return (
    <div className="group flex items-center gap-4 px-4 py-3">
      {icon ? (
        <img src={icon} alt="" className="size-12 shrink-0 rounded-lg [image-rendering:pixelated]" />
      ) : (
        <div className="flex size-12 shrink-0 items-center justify-center rounded-lg bg-muted">
          <Server className="size-5 text-muted-foreground" aria-hidden="true" />
        </div>
      )}
      <div className="min-w-0 flex-1 space-y-1">
        <div className="flex items-baseline gap-2">
          <p className="truncate text-sm font-semibold">{server.name}</p>
          <p className="truncate font-mono text-[0.7rem] text-muted-foreground">{server.address}</p>
        </div>
        {status.isLoading ? (
          <p className="flex items-center gap-1.5 text-xs text-muted-foreground">
            <Loader2 className="size-3 animate-spin" aria-hidden="true" /> Connexion…
          </p>
        ) : status.isError ? (
          <p className="text-xs text-destructive">Hors ligne — {errorMessage(status.error)}</p>
        ) : status.data ? (
          <div className="rounded-md bg-black/70 px-2 py-1" title={motdPlainText(status.data.motd)}>
            <Motd text={status.data.motd} />
          </div>
        ) : null}
      </div>
      {status.data && (
        <div className="flex shrink-0 flex-col items-end gap-1 text-xs">
          <span
            className="flex items-center gap-1 text-muted-foreground"
            title={status.data.players.length ? status.data.players.join(", ") : undefined}
          >
            <Users className="size-3.5" aria-hidden="true" />
            <span className="font-medium text-foreground tabular-nums">
              {status.data.online.toLocaleString("fr-FR")}
            </span>
            / {status.data.max.toLocaleString("fr-FR")}
          </span>
          <span className={cn("flex items-center gap-1 tabular-nums", latencyColor(status.data.latency_ms))}>
            <Signal className="size-3.5" aria-hidden="true" />
            {status.data.latency_ms} ms
          </span>
        </div>
      )}
      <div className="flex shrink-0 items-center gap-1">
        <Button
          variant="ghost"
          size="icon-sm"
          title="Modifier"
          aria-label={`Modifier ${server.name}`}
          className="opacity-0 transition-opacity group-hover:opacity-100 focus-visible:opacity-100"
          onClick={onEdit}
        >
          <Pencil aria-hidden="true" />
        </Button>
        <Button
          variant="ghost"
          size="icon-sm"
          title="Supprimer"
          aria-label={`Supprimer ${server.name}`}
          className="opacity-0 transition-opacity group-hover:opacity-100 focus-visible:opacity-100"
          onClick={onDelete}
        >
          <Trash2 aria-hidden="true" />
        </Button>
        <Button size="sm" className="ml-1 gap-1.5" onClick={onPlay}>
          <Play className="fill-current" aria-hidden="true" />
          Jouer
        </Button>
      </div>
    </div>
  );
}

interface EditTarget {
  /** null = new server. */
  index: number | null;
  name: string;
  address: string;
}

/** The instance's multiplayer list (`servers.dat`), live status, and one-click join. */
export function ServersTab({ instance }: { instance: Instance }) {
  const queryClient = useQueryClient();
  const playInstance = usePlayInstance();
  const [editing, setEditing] = useState<EditTarget | null>(null);
  const [deleting, setDeleting] = useState<number | null>(null);
  const queryKey = ["instance-servers", instance.id];

  const { data: servers, isLoading } = useQuery({ queryKey, queryFn: () => serversApi.list(instance.id) });

  const save = useMutation({
    mutationFn: (target: EditTarget) =>
      target.index === null
        ? serversApi.add(instance.id, target.name, target.address)
        : serversApi.update(instance.id, target.index, target.name, target.address),
    onSuccess: () => {
      setEditing(null);
      queryClient.invalidateQueries({ queryKey });
    },
    onError: (e) => notify.error({ title: "Enregistrement impossible", message: errorMessage(e), history: false }),
  });

  const remove = useMutation({
    mutationFn: (index: number) => serversApi.remove(instance.id, index),
    onSuccess: () => {
      setDeleting(null);
      queryClient.invalidateQueries({ queryKey });
    },
    onError: (e) => notify.error({ title: "Suppression impossible", message: errorMessage(e), history: false }),
  });

  const refresh = () => {
    queryClient.invalidateQueries({ queryKey });
    queryClient.invalidateQueries({ queryKey: ["server-ping"] });
  };

  return (
    <>
      <div className="mb-3 flex justify-end gap-2">
        <Button variant="ghost" size="sm" className="gap-1.5" onClick={refresh}>
          <RefreshCw aria-hidden="true" />
          Actualiser
        </Button>
        <Button size="sm" className="gap-1.5" onClick={() => setEditing({ index: null, name: "", address: "" })}>
          <Plus aria-hidden="true" />
          Ajouter un serveur
        </Button>
      </div>

      {isLoading ? (
        <div className="flex justify-center py-12">
          <Loader2 className="size-5 animate-spin text-muted-foreground" aria-hidden="true" />
        </div>
      ) : !servers?.length ? (
        <EmptyState
          icon={Server}
          title="Aucun serveur"
          description="Ajoute un serveur pour voir s'il est en ligne et le rejoindre en un clic. La liste est partagée avec le menu Multijoueur du jeu."
        />
      ) : (
        <div className="glass divide-y divide-border/60 rounded-2xl">
          {servers.map((server, index) => (
            <ServerRow
              key={`${index}-${server.address}`}
              server={server}
              onPlay={() => playInstance(instance, server.address)}
              onEdit={() => setEditing({ index, name: server.name, address: server.address })}
              onDelete={() => setDeleting(index)}
            />
          ))}
        </div>
      )}

      <Dialog open={editing !== null} onOpenChange={(open) => !open && setEditing(null)}>
        <DialogContent className="sm:max-w-md">
          <DialogHeader>
            <DialogTitle>{editing?.index === null ? "Ajouter un serveur" : "Modifier le serveur"}</DialogTitle>
            <DialogDescription>Il apparaîtra aussi dans le menu Multijoueur du jeu.</DialogDescription>
          </DialogHeader>
          {editing && (
            <form
              id="server-form"
              className="space-y-3"
              onSubmit={(e) => {
                e.preventDefault();
                save.mutate(editing);
              }}
            >
              <div className="space-y-1.5">
                <Label htmlFor="server-name">Nom</Label>
                <Input
                  id="server-name"
                  maxLength={64}
                  placeholder="Serveur Minecraft"
                  value={editing.name}
                  onChange={(e) => setEditing({ ...editing, name: e.target.value })}
                />
              </div>
              <div className="space-y-1.5">
                <Label htmlFor="server-address">Adresse</Label>
                <Input
                  id="server-address"
                  autoFocus
                  placeholder="play.exemple.fr"
                  value={editing.address}
                  onChange={(e) => setEditing({ ...editing, address: e.target.value })}
                />
              </div>
            </form>
          )}
          <DialogFooter>
            <Button variant="outline" onClick={() => setEditing(null)}>
              Annuler
            </Button>
            <Button
              type="submit"
              form="server-form"
              className="gap-1.5"
              disabled={!editing?.address.trim() || save.isPending}
            >
              {save.isPending && <Loader2 className="animate-spin" aria-hidden="true" />}
              Enregistrer
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      <ConfirmDialog
        open={deleting !== null}
        onOpenChange={(open) => !open && setDeleting(null)}
        title={`Retirer « ${deleting !== null ? servers?.[deleting]?.name : ""} » ?`}
        description="Le serveur disparaîtra aussi de la liste Multijoueur du jeu."
        confirmLabel="Retirer"
        destructive
        pending={remove.isPending}
        onConfirm={() => deleting !== null && remove.mutate(deleting)}
      />
    </>
  );
}
