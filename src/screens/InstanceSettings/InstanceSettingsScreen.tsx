import { useEffect, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useBlocker, useNavigate, useParams } from "react-router";
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
import { MemorySlider } from "@/components/MemorySlider";
import { PageHeader } from "@/components/PageHeader";
import { Textarea } from "@/components/ui/textarea";
import { useSettings } from "@/hooks/useSettings";
import { parseJvmArgs } from "@/lib/jvmArgs";
import { errorMessage, getSystemMemoryMb, instancesApi } from "@/services/tauri";

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
  const { data: settings } = useSettings();
  const { data: systemMemoryMb } = useQuery({ queryKey: ["system-memory"], queryFn: getSystemMemoryMb });

  const [minMb, setMinMb] = useState("");
  const [maxMb, setMaxMb] = useState("");
  const [jvmArgs, setJvmArgs] = useState("");
  const [initial, setInitial] = useState<{ minMb: string; maxMb: string; jvmArgs: string } | null>(null);

  useEffect(() => {
    if (!instance) return;
    const loadedMin = instance.min_memory_mb?.toString() ?? "";
    const loadedMax = instance.max_memory_mb?.toString() ?? "";
    const loadedJvmArgs = instance.extra_jvm_args.join(" ");
    setMinMb(loadedMin);
    setMaxMb(loadedMax);
    setJvmArgs(loadedJvmArgs);
    setInitial({ minMb: loadedMin, maxMb: loadedMax, jvmArgs: loadedJvmArgs });
  }, [instance]);

  const isDirty =
    !!initial && (initial.minMb !== minMb || initial.maxMb !== maxMb || initial.jvmArgs !== jvmArgs);
  const blocker = useBlocker(
    ({ currentLocation, nextLocation }) => isDirty && currentLocation.pathname !== nextLocation.pathname,
  );

  const saveMutation = useMutation({
    mutationFn: () =>
      instancesApi.updateSettings(
        instanceId,
        minMb ? Number(minMb) : null,
        maxMb ? Number(maxMb) : null,
        parseJvmArgs(jvmArgs),
      ),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["instance", instanceId] });
      setInitial({ minMb, maxMb, jvmArgs });
      toast.success("Paramètres enregistrés");
    },
    onError: (e) => toast.error(errorMessage(e)),
  });

  async function saveAndLeave() {
    try {
      await saveMutation.mutateAsync();
      blocker.proceed?.();
    } catch {
      // saveMutation.onError already toasted — stay on the page.
    }
  }

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
            <Label>RAM max.</Label>
            <MemorySlider
              valueMb={Number(maxMb) || settings?.default_max_memory_mb || 4096}
              onChangeMb={(v) => setMaxMb(String(v))}
              maxMb={systemMemoryMb ?? 16384}
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

      <Dialog open={blocker.state === "blocked"} onOpenChange={(open) => !open && blocker.reset?.()}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Modifications non enregistrées</DialogTitle>
            <DialogDescription>
              Tu as des changements non enregistrés sur cette instance. Les enregistrer avant de continuer ?
            </DialogDescription>
          </DialogHeader>
          <DialogFooter>
            <Button variant="outline" onClick={() => blocker.reset?.()}>
              Annuler
            </Button>
            <Button variant="outline" onClick={() => blocker.proceed?.()}>
              Ignorer les changements
            </Button>
            <Button onClick={saveAndLeave} disabled={saveMutation.isPending} className="gap-1.5">
              {saveMutation.isPending && <Loader2 className="size-3.5 animate-spin" aria-hidden="true" />}
              Enregistrer et continuer
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}
