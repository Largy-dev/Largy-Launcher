import { useMemo, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Loader2, MemoryStick } from "lucide-react";

import { LOADER_META, LOADER_ORDER } from "@/components/instance/LoaderBadge";
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
import { useSystemMemory } from "@/hooks/useInstanceInfo";
import { formatGb } from "@/lib/format";
import { notify } from "@/lib/notify";
import { adviseRam } from "@/lib/ramAdvice";
import { cn } from "@/lib/utils";
import { errorMessage, instancesApi, loadersListVersions, minecraftApi, type LoaderKind } from "@/services/tauri";

const LOADER_HINTS: Record<LoaderKind, string> = {
  vanilla: "Le jeu original",
  fabric: "Léger, mods de perf",
  quilt: "Fork de Fabric",
  forge: "Le classique",
  neoforge: "Forge moderne",
};

interface CreateInstanceDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

export function CreateInstanceDialog({ open, onOpenChange }: CreateInstanceDialogProps) {
  const queryClient = useQueryClient();
  const [name, setName] = useState("");
  const [minecraftVersion, setMinecraftVersion] = useState("");
  const [loader, setLoader] = useState<LoaderKind>("vanilla");
  const [loaderVersion, setLoaderVersion] = useState("");
  const { data: memory } = useSystemMemory();

  const versionsQuery = useQuery({
    queryKey: ["minecraft-versions"],
    queryFn: minecraftApi.listVersions,
    enabled: open,
  });

  const releaseVersions = useMemo(
    () => (versionsQuery.data ?? []).filter((v) => v.type === "release"),
    [versionsQuery.data],
  );

  const loaderVersionsQuery = useQuery({
    queryKey: ["loader-versions", loader, minecraftVersion],
    queryFn: () => loadersListVersions(loader, minecraftVersion),
    enabled: open && loader !== "vanilla" && minecraftVersion !== "",
  });

  const createMutation = useMutation({
    mutationFn: () =>
      instancesApi.create(
        name.trim() || minecraftVersion,
        minecraftVersion,
        loader,
        loader === "vanilla" ? null : loaderVersion || null,
      ),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["instances"] });
      notify.success({ title: "Instance créée", message: name.trim() || minecraftVersion });
      close();
    },
    onError: (e) => notify.error({ title: "Création impossible", message: errorMessage(e) }),
  });

  function close() {
    onOpenChange(false);
    setName("");
    setMinecraftVersion("");
    setLoader("vanilla");
    setLoaderVersion("");
  }

  const canCreate =
    minecraftVersion !== "" && (loader === "vanilla" || loaderVersion !== "") && !createMutation.isPending;

  return (
    <Dialog open={open} onOpenChange={(next) => (next ? onOpenChange(true) : close())}>
      <DialogContent className="sm:max-w-lg">
        <DialogHeader>
          <DialogTitle>Nouvelle instance</DialogTitle>
          <DialogDescription>Choisis une version de Minecraft et un mod loader.</DialogDescription>
        </DialogHeader>

        <div className="space-y-4">
          <div className="space-y-1.5">
            <Label htmlFor="instance-name">Nom</Label>
            <Input
              id="instance-name"
              placeholder={minecraftVersion || "Ma nouvelle instance"}
              value={name}
              onChange={(e) => setName(e.target.value)}
            />
          </div>

          <div className="space-y-1.5">
            <Label>Version de Minecraft</Label>
            {versionsQuery.isLoading ? (
              <p className="flex items-center gap-2 text-sm text-muted-foreground">
                <Loader2 className="size-3.5 animate-spin" aria-hidden="true" /> Chargement des versions…
              </p>
            ) : (
              <Select value={minecraftVersion} onValueChange={setMinecraftVersion}>
                <SelectTrigger className="w-full">
                  <SelectValue placeholder="Choisir une version" />
                </SelectTrigger>
                <SelectContent>
                  {releaseVersions.map((v) => (
                    <SelectItem key={v.id} value={v.id}>
                      {v.id}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            )}
          </div>

          <div className="space-y-1.5">
            <Label>Mod loader</Label>
            <div role="radiogroup" className="grid grid-cols-5 gap-2">
              {LOADER_ORDER.map((value) => {
                const meta = LOADER_META[value];
                const Icon = meta.icon;
                const selected = loader === value;
                return (
                  <button
                    key={value}
                    type="button"
                    role="radio"
                    aria-checked={selected}
                    title={LOADER_HINTS[value]}
                    onClick={() => {
                      setLoader(value);
                      setLoaderVersion("");
                    }}
                    className={cn(
                      "flex flex-col items-center gap-1.5 rounded-xl border-2 p-2.5 text-xs font-semibold transition-all hover:-translate-y-0.5",
                      selected ? "shadow-lg" : "border-border text-muted-foreground hover:text-foreground",
                    )}
                    style={
                      selected
                        ? {
                            borderColor: meta.color,
                            backgroundColor: `color-mix(in oklab, ${meta.color} 14%, transparent)`,
                          }
                        : undefined
                    }
                  >
                    <Icon className="size-5" style={{ color: meta.color }} aria-hidden="true" />
                    {meta.label}
                  </button>
                );
              })}
            </div>
            <p className="text-xs text-muted-foreground">{LOADER_HINTS[loader]}</p>
          </div>

          {loader !== "vanilla" && (
            <div className="space-y-1.5">
              <Label>Version de {LOADER_META[loader].label}</Label>
              {!minecraftVersion ? (
                <p className="text-sm text-muted-foreground">Choisis d'abord une version de Minecraft.</p>
              ) : loaderVersionsQuery.isLoading ? (
                <p className="flex items-center gap-2 text-sm text-muted-foreground">
                  <Loader2 className="size-3.5 animate-spin" aria-hidden="true" /> Chargement…
                </p>
              ) : loaderVersionsQuery.isError ? (
                <p className="text-sm text-destructive">{errorMessage(loaderVersionsQuery.error)}</p>
              ) : (
                <Select value={loaderVersion} onValueChange={setLoaderVersion}>
                  <SelectTrigger className="w-full">
                    <SelectValue placeholder="Choisir une version" />
                  </SelectTrigger>
                  <SelectContent>
                    {(loaderVersionsQuery.data ?? []).map((v) => (
                      <SelectItem key={v} value={v}>
                        {v}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              )}
            </div>
          )}
          {minecraftVersion && (
            <p className="flex items-center gap-1.5 rounded-lg bg-muted/60 px-3 py-2 text-xs text-muted-foreground">
              <MemoryStick className="size-3.5" aria-hidden="true" />
              RAM conseillée pour démarrer :{" "}
              <span className="font-semibold text-foreground">
                {formatGb(
                  adviseRam({
                    loader,
                    modCount: loader === "vanilla" ? null : 0,
                    minecraftVersion,
                    isModpack: false,
                    systemTotalMb: memory?.total_mb ?? null,
                  }).recommendedMb,
                )}
              </span>
              — ajustable ensuite selon tes mods.
            </p>
          )}
        </div>

        <DialogFooter>
          <Button variant="outline" onClick={close}>
            Annuler
          </Button>
          <Button onClick={() => createMutation.mutate()} disabled={!canCreate} className="bg-gradient-brand gap-1.5">
            {createMutation.isPending && <Loader2 className="size-3.5 animate-spin" aria-hidden="true" />}
            Créer
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
