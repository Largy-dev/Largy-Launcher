import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useNavigate, useParams } from "react-router";
import { open } from "@tauri-apps/plugin-dialog";
import { Blocks, Loader2, Plus, Trash2 } from "lucide-react";
import { toast } from "sonner";

import { Button } from "@/components/ui/button";
import { Switch } from "@/components/ui/switch";
import { EmptyState } from "@/components/EmptyState";
import { PageHeader } from "@/components/PageHeader";
import { errorMessage, instanceModsApi, instancesApi } from "@/services/tauri";

function formatSize(bytes: number): string {
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(0)} Ko`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} Mo`;
}

export function InstanceModsScreen() {
  const { id } = useParams<{ id: string }>();
  const instanceId = id ?? "";
  const navigate = useNavigate();
  const queryClient = useQueryClient();

  const { data: instance } = useQuery({
    queryKey: ["instance", instanceId],
    queryFn: () => instancesApi.get(instanceId),
    enabled: instanceId !== "",
  });

  const modsQuery = useQuery({
    queryKey: ["instance-mods", instanceId],
    queryFn: () => instanceModsApi.list(instanceId),
    enabled: instanceId !== "",
  });

  function invalidate() {
    queryClient.invalidateQueries({ queryKey: ["instance-mods", instanceId] });
  }

  const toggleMutation = useMutation({
    mutationFn: (vars: { fileName: string; enabled: boolean }) =>
      instanceModsApi.setEnabled(instanceId, vars.fileName, vars.enabled),
    onSuccess: invalidate,
    onError: (e) => toast.error(errorMessage(e)),
  });

  const deleteMutation = useMutation({
    mutationFn: (fileName: string) => instanceModsApi.delete(instanceId, fileName),
    onSuccess: invalidate,
    onError: (e) => toast.error(errorMessage(e)),
  });

  const addMutation = useMutation({
    mutationFn: (sourcePath: string) => instanceModsApi.add(instanceId, sourcePath),
    onSuccess: invalidate,
    onError: (e) => toast.error(errorMessage(e)),
  });

  async function addMod() {
    const picked = await open({
      multiple: false,
      filters: [{ name: "Mod (.jar)", extensions: ["jar"] }],
    });
    if (typeof picked === "string") {
      addMutation.mutate(picked);
    }
  }

  const mods = modsQuery.data ?? [];

  return (
    <div className="flex flex-1 flex-col">
      <PageHeader
        title={instance ? `Mods — ${instance.name}` : "Mods"}
        description="Active, désactive, supprime ou ajoute des mods pour cette instance."
        action={
          <div className="flex gap-2">
            <Button variant="outline" size="sm" onClick={() => navigate(`/instances/${instanceId}`)}>
              Retour
            </Button>
            <Button size="sm" onClick={addMod} disabled={addMutation.isPending} className="gap-1.5">
              {addMutation.isPending ? (
                <Loader2 className="size-3.5 animate-spin" aria-hidden="true" />
              ) : (
                <Plus className="size-3.5" aria-hidden="true" />
              )}
              Ajouter un mod…
            </Button>
          </div>
        }
      />

      {modsQuery.isLoading ? (
        <div className="flex flex-1 items-center justify-center">
          <Loader2 className="size-5 animate-spin text-muted-foreground" aria-hidden="true" />
        </div>
      ) : mods.length === 0 ? (
        <EmptyState
          icon={Blocks}
          title="Aucun mod"
          description="Ajoute un fichier .jar pour l'installer dans cette instance."
        />
      ) : (
        <div className="divide-y divide-border rounded-lg border border-border">
          {mods.map((mod) => (
            <div key={mod.file_name} className="flex items-center justify-between gap-4 px-4 py-3">
              <div className="min-w-0 flex-1">
                <p className="truncate text-sm font-medium">{mod.file_name}</p>
                <p className="text-xs text-muted-foreground">{formatSize(mod.size)}</p>
              </div>
              <div className="flex items-center gap-3">
                <Switch
                  checked={mod.enabled}
                  onCheckedChange={(enabled) => toggleMutation.mutate({ fileName: mod.file_name, enabled })}
                />
                <Button
                  variant="ghost"
                  size="icon-sm"
                  title="Supprimer"
                  onClick={() => deleteMutation.mutate(mod.file_name)}
                >
                  <Trash2 className="size-4" aria-hidden="true" />
                </Button>
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
