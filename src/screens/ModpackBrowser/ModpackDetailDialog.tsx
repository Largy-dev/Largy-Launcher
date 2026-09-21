import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useNavigate } from "react-router";
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
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { errorMessage, instancesApi, providersApi, type ModpackSummary, type ProviderId } from "@/services/tauri";
import { useAppStore } from "@/store/appStore";

interface ModpackDetailDialogProps {
  provider: ProviderId;
  pack: ModpackSummary | null;
  onOpenChange: (open: boolean) => void;
}

export function ModpackDetailDialog({ provider, pack, onOpenChange }: ModpackDetailDialogProps) {
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const progress = useAppStore((s) => s.downloadProgress);
  const [versionId, setVersionId] = useState("");
  const [instanceName, setInstanceName] = useState("");

  const versionsQuery = useQuery({
    queryKey: ["modpack-versions", provider, pack?.id],
    queryFn: () => providersApi.getVersions(provider, pack!.id),
    enabled: pack !== null,
  });

  const installMutation = useMutation({
    mutationFn: () =>
      instancesApi.installModpack(
        provider,
        pack!.id,
        versionId,
        pack!.name,
        instanceName.trim() || pack!.name,
      ),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["instances"] });
      toast.success(`${pack?.name} installé`);
      close();
      navigate("/");
    },
    onError: (e) => toast.error(errorMessage(e)),
  });

  function close() {
    onOpenChange(false);
    setVersionId("");
    setInstanceName("");
  }

  const percent =
    installMutation.isPending && progress && progress.bytes_total > 0
      ? Math.min(100, Math.round((progress.bytes_done / progress.bytes_total) * 100))
      : null;

  return (
    <Dialog open={pack !== null} onOpenChange={(next) => (next ? undefined : close())}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>{pack?.name}</DialogTitle>
          <DialogDescription>{pack?.summary}</DialogDescription>
        </DialogHeader>

        <div className="space-y-4">
          <div className="space-y-1.5">
            <Label htmlFor="modpack-instance-name">Nom de l'instance</Label>
            <Input
              id="modpack-instance-name"
              placeholder={pack?.name}
              value={instanceName}
              onChange={(e) => setInstanceName(e.target.value)}
            />
          </div>

          <div className="space-y-1.5">
            <Label>Version</Label>
            {versionsQuery.isLoading ? (
              <p className="flex items-center gap-2 text-sm text-muted-foreground">
                <Loader2 className="size-3.5 animate-spin" aria-hidden="true" /> Chargement…
              </p>
            ) : (
              <Select value={versionId} onValueChange={setVersionId}>
                <SelectTrigger className="w-full">
                  <SelectValue placeholder="Choisir une version" />
                </SelectTrigger>
                <SelectContent>
                  {(versionsQuery.data ?? []).map((v) => (
                    <SelectItem key={v.id} value={v.id}>
                      {v.name} ({v.minecraft_version}
                      {v.loader !== "vanilla" ? ` · ${v.loader}` : ""})
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            )}
          </div>

          {installMutation.isPending && (
            <p className="flex items-center gap-2 text-sm text-muted-foreground">
              <Loader2 className="size-3.5 animate-spin" aria-hidden="true" />
              Installation{percent !== null ? ` — ${percent}%` : "…"}
            </p>
          )}
        </div>

        <DialogFooter>
          <Button variant="outline" onClick={close} disabled={installMutation.isPending}>
            Annuler
          </Button>
          <Button
            onClick={() => installMutation.mutate()}
            disabled={!versionId || installMutation.isPending}
            className="gap-1.5"
          >
            {installMutation.isPending && <Loader2 className="size-3.5 animate-spin" aria-hidden="true" />}
            Installer
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
