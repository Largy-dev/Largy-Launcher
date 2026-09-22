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
import {
  errorMessage,
  instancesApi,
  providersApi,
  type InstanceInstallResult,
  type ModpackSummary,
  type ProviderId,
} from "@/services/tauri";

interface ModpackDetailDialogProps {
  provider: ProviderId;
  pack: ModpackSummary | null;
  onOpenChange: (open: boolean) => void;
  /** When set, the dialog installs the chosen version into this existing
   * instance instead of creating a new one — used for the "update available"
   * flow on an already-installed modpack. */
  updateInstanceId?: string;
}

interface InstallVars {
  packId: string;
  versionId: string;
  packName: string;
  packIconUrl: string | null;
  instanceName: string;
}

function notifyResult(result: InstanceInstallResult, successMessage: string) {
  toast.success(successMessage);
  if (result.warnings.length > 0) {
    toast.warning(`${result.warnings.length} avertissement(s) lors de l'installation`, {
      description: result.warnings.slice(0, 3).join("\n"),
    });
  }
}

export function ModpackDetailDialog({ provider, pack, onOpenChange, updateInstanceId }: ModpackDetailDialogProps) {
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const [versionId, setVersionId] = useState("");
  const [instanceName, setInstanceName] = useState("");
  const isUpdate = !!updateInstanceId;

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
      notifyResult(result, `${vars.packName} installé`);
    },
    onError: (e) => toast.error(errorMessage(e)),
  });

  const updateMutation = useMutation({
    mutationFn: (vars: { versionId: string }) => instancesApi.updateModpack(updateInstanceId!, vars.versionId),
    onSuccess: (result) => {
      queryClient.invalidateQueries({ queryKey: ["instances"] });
      queryClient.invalidateQueries({ queryKey: ["modpack-versions", provider, pack?.id] });
      notifyResult(result, `${pack?.name ?? "Modpack"} mis à jour`);
    },
    onError: (e) => toast.error(errorMessage(e)),
  });

  const activeMutation = isUpdate ? updateMutation : installMutation;

  function close() {
    onOpenChange(false);
    setVersionId("");
    setInstanceName("");
  }

  function submitInstall() {
    if (!pack) return;
    if (isUpdate) {
      updateMutation.mutate({ versionId });
    } else {
      installMutation.mutate({
        packId: pack.id,
        versionId,
        packName: pack.name,
        packIconUrl: pack.icon_url,
        instanceName: instanceName.trim() || pack.name,
      });
    }
    close();
    if (!isUpdate) navigate("/");
  }

  return (
    <Dialog open={pack !== null} onOpenChange={(next) => (next ? undefined : close())}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>{pack?.name}</DialogTitle>
          <DialogDescription>
            {isUpdate ? "Choisis la version vers laquelle mettre à jour cette instance." : pack?.summary}
          </DialogDescription>
        </DialogHeader>

        <div className="space-y-4">
          {!isUpdate && (
            <div className="space-y-1.5">
              <Label htmlFor="modpack-instance-name">Nom de l'instance</Label>
              <Input
                id="modpack-instance-name"
                placeholder={pack?.name}
                value={instanceName}
                onChange={(e) => setInstanceName(e.target.value)}
              />
            </div>
          )}

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
          <Button onClick={submitInstall} disabled={!versionId || activeMutation.isPending} className="gap-1.5">
            {activeMutation.isPending && <Loader2 className="size-3.5 animate-spin" aria-hidden="true" />}
            {isUpdate ? "Mettre à jour" : "Installer"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
