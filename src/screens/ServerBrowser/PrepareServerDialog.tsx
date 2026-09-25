import { useEffect, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Check, Loader2, Lock, Package, ShieldAlert } from "lucide-react";

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
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { usePlayInstance } from "@/hooks/useLaunchInstance";
import { useServerStatus } from "@/hooks/useServerStatus";
import { notify } from "@/lib/notify";
import { cn } from "@/lib/utils";
import { notifyInstallResult } from "@/lib/installResult";
import { catalogApi, type FeaturedServer, type ServerPreset } from "@/services/servers";
import { errorMessage, instancesApi, isCancelled, minecraftApi, providersApi } from "@/services/tauri";

/** What the dialog prepares: a catalog server, or one the player types in (`null`). */
export type PrepareTarget = { server: FeaturedServer } | { server: null };

interface PrepareServerDialogProps {
  target: PrepareTarget | null;
  presets: ServerPreset[];
  onOpenChange: (open: boolean) => void;
}

function PresetToggle({ preset, on, onToggle }: { preset: ServerPreset; on: boolean; onToggle: () => void }) {
  return (
    <button
      type="button"
      onClick={onToggle}
      aria-pressed={on}
      className={cn(
        "flex w-full items-start gap-3 rounded-xl p-3 text-left ring-1 ring-border/60 transition",
        on ? "bg-accent ring-primary/60" : "hover:bg-accent/50",
      )}
    >
      <span
        className={cn(
          "mt-0.5 flex size-4 shrink-0 items-center justify-center rounded border",
          on ? "border-primary bg-primary text-primary-foreground" : "border-muted-foreground/40",
        )}
      >
        {on && <Check className="size-3" aria-hidden="true" />}
      </span>
      <span className="min-w-0">
        <span className="block text-sm font-medium">{preset.label}</span>
        <span className="block text-xs text-muted-foreground">{preset.description}</span>
      </span>
    </button>
  );
}

/** Pick the version, name and mod packs, then build an instance that joins the server directly. */
export function PrepareServerDialog({ target, presets, onOpenChange }: PrepareServerDialogProps) {
  const queryClient = useQueryClient();
  const playInstance = usePlayInstance();
  const featured = target?.server ?? null;
  const custom = target !== null && featured === null;

  const [name, setName] = useState("");
  const [address, setAddress] = useState("");
  const [version, setVersion] = useState("");
  const [chosen, setChosen] = useState<Set<string>>(new Set());

  useEffect(() => {
    if (!target) return;
    // Seeds the form each time the dialog opens on a new server.
    // eslint-disable-next-line react-hooks/set-state-in-effect
    setName(target.server?.name ?? "");
    setAddress(target.server?.address ?? "");
    setVersion(target.server?.minecraft_version ?? "");
    setChosen(new Set(presets.filter((p) => p.default).map((p) => p.id)));
  }, [target, presets]);

  const versions = useQuery({
    queryKey: ["minecraft-versions"],
    queryFn: minecraftApi.listVersions,
    enabled: custom,
    staleTime: 60 * 60 * 1000,
  });
  const releases = (versions.data ?? []).filter((v) => v.type === "release").slice(0, 40);
  const status = useServerStatus(featured?.address ?? "");

  const prepare = useMutation({
    mutationFn: () => {
      const picked = presets.filter((p) => chosen.has(p.id));
      return catalogApi.prepare({
        featured_id: featured?.id ?? "custom",
        name: name.trim() || featured?.name || address.trim(),
        address: address.trim(),
        minecraft_version: version,
        mods: [...(featured?.required_mods ?? []), ...picked.flatMap((p) => p.mods)],
        shaders: picked.flatMap((p) => p.shaders),
        icon: featured ? (status.data?.favicon ?? null) : null,
      });
    },
    onSuccess: (result) => {
      onOpenChange(false);
      queryClient.invalidateQueries({ queryKey: ["instances"] });
      const title = `${result.instance.name} est prêt`;
      const play = { label: "Jouer", onClick: () => playInstance(result.instance) };
      if (result.warnings.length) {
        notify.warning({
          title,
          message: `Certains mods n'ont pas pu être installés : ${result.warnings.join(" · ")}`,
          action: play,
        });
      } else {
        notify.success({ title, message: "Le jeu rejoindra directement le serveur.", action: play });
      }
    },
    onError: (e) => notify.error({ title: "Préparation impossible", message: errorMessage(e) }),
  });

  // Modded servers: install the exact modpack version, then link the server.
  // The dialog closes right away; progress shows on the instance card.
  const installPack = useMutation({
    mutationFn: async (vars: { server: FeaturedServer; name: string }) => {
      const pack = vars.server.modpack!;
      const icon = await providersApi
        .getModpack(pack.provider, pack.pack_id)
        .then((d) => d.summary.icon_url)
        .catch(() => null);
      const result = await instancesApi.installModpack(
        pack.provider,
        pack.pack_id,
        pack.version_id,
        pack.name,
        icon,
        vars.name,
      );
      const attached = await catalogApi.attach(result.instance.id, vars.server.id, vars.server.address);
      return { instance: attached.instance, warnings: [...result.warnings, ...attached.warnings] };
    },
    onSuccess: (result, vars) => {
      queryClient.invalidateQueries({ queryKey: ["instances"] });
      notifyInstallResult(result, `${result.instance.name} est prêt pour ${vars.server.name}`, () =>
        playInstance(result.instance),
      );
    },
    onError: (e, vars) =>
      isCancelled(e)
        ? notify.info({ title: `Installation pour ${vars.server.name} annulée`, history: false })
        : notify.error({ title: "Installation du modpack impossible", message: errorMessage(e) }),
  });

  const toggle = (id: string) =>
    setChosen((current) => {
      const next = new Set(current);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });

  const ready = !!version && !!address.trim() && !prepare.isPending;
  const modpack = featured?.modpack ?? null;

  function submit() {
    if (featured && modpack) {
      installPack.mutate({ server: featured, name: name.trim() || featured.name });
      onOpenChange(false);
      notify.info({
        title: `Installation de ${modpack.name}`,
        message: "Suis la progression sur la carte de l'instance. Tu seras prévenu quand tout est prêt.",
        history: false,
      });
    } else {
      prepare.mutate();
    }
  }

  return (
    <Dialog open={target !== null} onOpenChange={(open) => !prepare.isPending && onOpenChange(open)}>
      <DialogContent className="sm:max-w-lg">
        <DialogHeader>
          <DialogTitle>{featured ? `Jouer sur ${featured.name}` : "Ajouter mon serveur"}</DialogTitle>
          <DialogDescription>
            {featured?.modpack
              ? "Une instance dédiée est créée avec le modpack du serveur, et le jeu s'y connecte directement."
              : "Une instance Fabric dédiée est créée : bonne version, mods choisis, et le jeu se connecte directement au serveur."}
          </DialogDescription>
        </DialogHeader>

        <div className="space-y-4">
          {custom && (
            <div className="space-y-1.5">
              <Label htmlFor="prepare-address">Adresse du serveur</Label>
              <Input
                id="prepare-address"
                autoFocus
                placeholder="play.exemple.fr"
                value={address}
                onChange={(e) => setAddress(e.target.value)}
              />
            </div>
          )}
          <div className="grid grid-cols-2 gap-3">
            <div className="space-y-1.5">
              <Label htmlFor="prepare-name">Nom de l'instance</Label>
              <Input
                id="prepare-name"
                maxLength={64}
                placeholder={featured?.name ?? "Mon serveur"}
                value={name}
                onChange={(e) => setName(e.target.value)}
              />
            </div>
            <div className="space-y-1.5">
              <Label>Version de Minecraft</Label>
              {modpack ? (
                <p className="flex h-9 items-center text-sm">{version}</p>
              ) : custom ? (
                <Select value={version} onValueChange={setVersion}>
                  <SelectTrigger className="w-full">
                    <SelectValue placeholder={versions.isLoading ? "Chargement…" : "Choisir"} />
                  </SelectTrigger>
                  <SelectContent>
                    {releases.map((v) => (
                      <SelectItem key={v.id} value={v.id}>
                        {v.id}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              ) : (
                <p className="flex h-9 items-center text-sm">
                  {version} <span className="ml-1.5 text-xs text-muted-foreground">(conseillée)</span>
                </p>
              )}
            </div>
          </div>

          {modpack ? (
            <div className="flex items-start gap-3 rounded-xl bg-accent/60 p-3 ring-1 ring-border/60">
              <Package className="mt-0.5 size-5 shrink-0 text-primary" aria-hidden="true" />
              <div className="min-w-0 text-sm">
                <p className="font-medium">
                  {modpack.name} <span className="text-muted-foreground">— version {modpack.version_name}</span>
                </p>
                <p className="text-xs text-muted-foreground">
                  Serveur moddé : le modpack est installé dans la version exacte du serveur, puis le jeu s'y connecte
                  directement.
                </p>
              </div>
            </div>
          ) : (
            <div className="space-y-2">
              <Label>Mods à installer</Label>
              {featured && featured.required_mods.length > 0 && (
                <p className="flex items-center gap-1.5 text-xs text-muted-foreground">
                  <Lock className="size-3" aria-hidden="true" />
                  Requis par le serveur : {featured.required_mods.join(", ")}
                </p>
              )}
              {presets.map((preset) => (
                <PresetToggle
                  key={preset.id}
                  preset={preset}
                  on={chosen.has(preset.id)}
                  onToggle={() => toggle(preset.id)}
                />
              ))}
              <p className="flex items-start gap-1.5 text-xs text-muted-foreground">
                <ShieldAlert className="mt-0.5 size-3.5 shrink-0" aria-hidden="true" />
                Ces mods ne touchent qu'à l'affichage et au confort : ils sont acceptés par les grands serveurs. Vérifie
                le règlement d'un serveur avant d'y ajouter d'autres mods.
              </p>
            </div>
          )}
        </div>

        <DialogFooter>
          <Button variant="outline" disabled={prepare.isPending} onClick={() => onOpenChange(false)}>
            Annuler
          </Button>
          <Button className="bg-gradient-brand shadow-glow gap-1.5" disabled={!ready} onClick={submit}>
            {prepare.isPending && <Loader2 className="animate-spin" aria-hidden="true" />}
            {prepare.isPending ? "Installation des mods…" : modpack ? "Installer le modpack" : "Préparer l'instance"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
