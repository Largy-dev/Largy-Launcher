import { useMemo, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Loader2 } from "lucide-react";
import { toast } from "sonner";

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
import { errorMessage, instancesApi, loadersListVersions, minecraftApi, type LoaderKind } from "@/services/tauri";

const LOADERS: { value: LoaderKind; label: string }[] = [
  { value: "vanilla", label: "Vanilla" },
  { value: "fabric", label: "Fabric" },
  { value: "quilt", label: "Quilt" },
  { value: "forge", label: "Forge" },
  { value: "neoforge", label: "NeoForge" },
];

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
      toast.success("Instance créée");
      close();
    },
    onError: (e) => toast.error(errorMessage(e)),
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
      <DialogContent className="sm:max-w-md">
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
            <Select
              value={loader}
              onValueChange={(v) => {
                setLoader(v as LoaderKind);
                setLoaderVersion("");
              }}
            >
              <SelectTrigger className="w-full">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {LOADERS.map((l) => (
                  <SelectItem key={l.value} value={l.value}>
                    {l.label}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>

          {loader !== "vanilla" && (
            <div className="space-y-1.5">
              <Label>Version de {LOADERS.find((l) => l.value === loader)?.label}</Label>
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
        </div>

        <DialogFooter>
          <Button variant="outline" onClick={close}>
            Annuler
          </Button>
          <Button onClick={() => createMutation.mutate()} disabled={!canCreate} className="gap-1.5">
            {createMutation.isPending && <Loader2 className="size-3.5 animate-spin" aria-hidden="true" />}
            Créer
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
