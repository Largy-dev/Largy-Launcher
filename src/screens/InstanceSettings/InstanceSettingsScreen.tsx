import { useEffect, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useNavigate, useParams } from "react-router";
import { Loader2 } from "lucide-react";
import { toast } from "sonner";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { PageHeader } from "@/components/PageHeader";
import { Textarea } from "@/components/ui/textarea";
import { errorMessage, instancesApi, settingsApi } from "@/services/tauri";

export function InstanceSettingsScreen() {
  const { id } = useParams<{ id: string }>();
  const instanceId = id ?? "";
  const navigate = useNavigate();
  const queryClient = useQueryClient();

  const { data: instance, isLoading } = useQuery({
    queryKey: ["instance", instanceId],
    queryFn: () => instancesApi.get(instanceId),
    enabled: instanceId !== "",
  });
  const { data: settings } = useQuery({ queryKey: ["settings"], queryFn: settingsApi.get });

  const [minMb, setMinMb] = useState("");
  const [maxMb, setMaxMb] = useState("");
  const [jvmArgs, setJvmArgs] = useState("");

  useEffect(() => {
    if (!instance) return;
    setMinMb(instance.min_memory_mb?.toString() ?? "");
    setMaxMb(instance.max_memory_mb?.toString() ?? "");
    setJvmArgs(instance.extra_jvm_args.join(" "));
  }, [instance]);

  const saveMutation = useMutation({
    mutationFn: () =>
      instancesApi.updateSettings(
        instanceId,
        minMb ? Number(minMb) : null,
        maxMb ? Number(maxMb) : null,
        jvmArgs.trim() ? jvmArgs.trim().split(/\s+/) : [],
      ),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["instance", instanceId] });
      toast.success("Paramètres enregistrés");
    },
    onError: (e) => toast.error(errorMessage(e)),
  });

  if (isLoading || !instance) {
    return (
      <div className="flex flex-1 items-center justify-center">
        <Loader2 className="size-5 animate-spin text-muted-foreground" aria-hidden="true" />
      </div>
    );
  }

  return (
    <div className="flex flex-1 flex-col">
      <PageHeader
        title={instance.name}
        description={`${instance.minecraft_version}${
          instance.loader !== "vanilla" ? ` · ${instance.loader}${instance.loader_version ? ` ${instance.loader_version}` : ""}` : ""
        }`}
        action={
          <Button variant="outline" size="sm" onClick={() => navigate("/")}>
            Retour
          </Button>
        }
      />

      <div className="max-w-md space-y-5">
        <div className="grid grid-cols-2 gap-4">
          <div className="space-y-1.5">
            <Label htmlFor="min-mb">RAM min. (Mo)</Label>
            <Input
              id="min-mb"
              type="number"
              placeholder={String(settings?.default_min_memory_mb ?? 1024)}
              value={minMb}
              onChange={(e) => setMinMb(e.target.value)}
            />
          </div>
          <div className="space-y-1.5">
            <Label htmlFor="max-mb">RAM max. (Mo)</Label>
            <Input
              id="max-mb"
              type="number"
              placeholder={String(settings?.default_max_memory_mb ?? 4096)}
              value={maxMb}
              onChange={(e) => setMaxMb(e.target.value)}
            />
          </div>
        </div>

        <div className="space-y-1.5">
          <Label htmlFor="jvm-args">Arguments JVM additionnels</Label>
          <Textarea
            id="jvm-args"
            placeholder="-Dfoo=bar"
            value={jvmArgs}
            onChange={(e) => setJvmArgs(e.target.value)}
          />
        </div>

        <Button onClick={() => saveMutation.mutate()} disabled={saveMutation.isPending} className="gap-1.5">
          {saveMutation.isPending && <Loader2 className="size-3.5 animate-spin" aria-hidden="true" />}
          Enregistrer
        </Button>
      </div>
    </div>
  );
}
