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
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { errorMessage, instancesApi, providersApi, type ModpackSummary, type ProviderId } from "@/services/tauri";

interface ModpackDetailDialogProps {
  provider: ProviderId;
  pack: ModpackSummary | null;
  onOpenChange: (open: boolean) => void;
}

interface InstallVars {
  packId: string;
  versionId: string;
  packName: string;
  packIconUrl: string | null;
  instanceName: string;
}

export function ModpackDetailDialog({ provider, pack, onOpenChange }: ModpackDetailDialogProps) {
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const [versionId, setVersionId] = useState("");
  const [instanceName, setInstanceName] = useState("");

  const versionsQuery = useQuery({
    queryKey: ["modpack-versions", provider, pack?.id],
    queryFn: () => providersApi.getVersions(provider, pack!.id),
    enabled: pack !== null,
  });

  // Variables are captured explicitly (not read from `pack`/`versionId` at
  // success time) since the dialog closes and navigates away immediately on
  // submit — by the time this resolves, `pack` may already be null.
  const installMutation = useMutation({
    mutationFn: (vars: InstallVars) =>
      instancesApi.installModpack(
        provider,
        vars.packId,
        vars.versionId,
        vars.packName,
        vars.packIconUrl,
        vars.instanceName,
      ),
    onSuccess: (result, vars) => {
      queryClient.invalidateQueries({ queryKey: ["instances"] });
      toast.success(`${vars.packName} installé`);
      if (result.warnings.length > 0) {
        toast.warning(`${result.warnings.length} avertissement(s) lors de l'installation`, {
          description: result.warnings.slice(0, 3).join("\n"),
        });
      }
    },
    onError: (e) => toast.error(errorMessage(e)),
  });

  function close() {
    onOpenChange(false);
    setVersionId("");
    setInstanceName("");
  }

  function submitInstall() {
    if (!pack) return;
    installMutation.mutate({
      packId: pack.id,
      versionId,
      packName: pack.name,
      packIconUrl: pack.icon_url,
      instanceName: instanceName.trim() || pack.name,
    });
    close();
    navigate("/");
  }

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
        </div>

        <DialogFooter>
          <Button variant="outline" onClick={close}>
            Annuler
          </Button>
          <Button onClick={submitInstall} disabled={!versionId} className="gap-1.5">
            Installer
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
