import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { CircleCheck, CircleX, Loader2, Wrench } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { notify } from "@/lib/notify";
import { cn } from "@/lib/utils";
import { errorMessage, instancesApi, isCancelled, launchApi, type Instance } from "@/services/tauri";
import { runtimeOf, useAppStore } from "@/store/appStore";

type StepState = "idle" | "running" | "done" | "error";

interface Step {
  id: string;
  title: string;
  description: string;
}

const GAME_STEP: Step = {
  id: "game",
  title: "Vérifier Minecraft et le loader",
  description: "Le jeu, le mod loader, les librairies et les ressources sont recontrôlés un par un.",
};

const PACK_STEP: Step = {
  id: "pack",
  title: "Vérifier les fichiers du modpack",
  description:
    "Chaque mod est comparé à la version installée et retéléchargé s'il manque ou est abîmé. Tes réglages et tes mondes ne sont pas touchés.",
};

/** Ids of the known issues that the modpack step already takes care of. */
const COVERED_BY_PACK_STEP = new Set(["missing-modpack-files"]);

interface RepairDialogProps {
  instance: Instance;
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

/**
 * "Réparer": the known problems the launcher spotted in this instance, each
 * with its fix, then a full check of the game and the modpack files.
 */
export function RepairDialog({ instance, open, onOpenChange }: RepairDialogProps) {
  const queryClient = useQueryClient();
  const running = useAppStore((s) => runtimeOf(s.runtime, instance.id).running);
  const setInstallWarnings = useAppStore((s) => s.setInstallWarnings);
  const [skipped, setSkipped] = useState<Set<string>>(new Set());
  const [progress, setProgress] = useState<Record<string, StepState>>({});

  const { data: issues, isLoading } = useQuery({
    queryKey: ["diagnose", instance.id],
    queryFn: () => instancesApi.diagnose(instance.id),
    enabled: open,
    staleTime: 0,
  });

  const steps: Step[] = [
    ...(issues ?? [])
      .filter((i) => !COVERED_BY_PACK_STEP.has(i.id))
      .map((i) => ({ id: i.id, title: i.title, description: i.description })),
    GAME_STEP,
    ...(instance.modpack ? [PACK_STEP] : []),
  ];
  const packIssue = issues?.find((i) => COVERED_BY_PACK_STEP.has(i.id));

  const repair = useMutation({
    mutationFn: async () => {
      const set = (id: string, state: StepState) => setProgress((p) => ({ ...p, [id]: state }));
      let failures = 0;
      for (const step of steps.filter((s) => !skipped.has(s.id))) {
        set(step.id, "running");
        try {
          if (step.id === GAME_STEP.id) {
            await launchApi.repair(instance.id);
          } else if (step.id === PACK_STEP.id) {
            const warnings = await instancesApi.repairModpackFiles(instance.id);
            if (warnings.length > 0) {
              setInstallWarnings({ instanceId: instance.id, instanceName: instance.name, warnings });
            }
          } else {
            await instancesApi.fixIssue(instance.id, step.id);
          }
          set(step.id, "done");
        } catch (e) {
          if (isCancelled(e)) throw e;
          failures++;
          set(step.id, "error");
          notify.error({ title: step.title, message: errorMessage(e), history: false });
        }
      }
      return failures;
    },
    onSuccess: (failures) => {
      queryClient.invalidateQueries({ queryKey: ["diagnose", instance.id] });
      queryClient.invalidateQueries({ queryKey: ["installed", instance.id] });
      queryClient.invalidateQueries({ queryKey: ["content-summary", instance.id] });
      if (failures === 0) {
        notify.success({ title: `${instance.name} est réparée`, message: "Tout a été vérifié et corrigé." });
      }
    },
  });

  const busy = repair.isPending;
  const finished = repair.isSuccess;

  return (
    <Dialog
      open={open}
      onOpenChange={(o) => {
        if (busy) return;
        onOpenChange(o);
        if (!o) {
          repair.reset();
          setProgress({});
          setSkipped(new Set());
        }
      }}
    >
      <DialogContent className="sm:max-w-lg">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <Wrench className="size-4 text-primary" aria-hidden="true" />
            Réparer {instance.name}
          </DialogTitle>
          <DialogDescription>
            {isLoading
              ? "Recherche des problèmes connus…"
              : issues && issues.length > 0
                ? `${issues.length} problème${issues.length > 1 ? "s" : ""} détecté${issues.length > 1 ? "s" : ""}. Décoche ce que tu ne veux pas corriger.`
                : "Aucun problème connu détecté. La vérification complète peut quand même régler un fichier abîmé."}
          </DialogDescription>
        </DialogHeader>

        {packIssue && (
          <p className="rounded-lg border border-warning/30 bg-warning/8 px-3 py-2 text-sm text-warning">
            {packIssue.title} — ils seront retéléchargés.
          </p>
        )}

        <ul className="space-y-1.5">
          {steps.map((step) => {
            const state = progress[step.id] ?? "idle";
            const isIssue = step.id !== GAME_STEP.id && step.id !== PACK_STEP.id;
            return (
              <li
                key={step.id}
                className={cn(
                  "flex gap-3 rounded-lg border border-border px-3 py-2.5",
                  isIssue && "border-primary/30 bg-primary/5",
                )}
              >
                <span className="mt-0.5 flex size-4 shrink-0 items-center justify-center">
                  {state === "running" ? (
                    <Loader2 className="size-4 animate-spin text-primary" aria-hidden="true" />
                  ) : state === "done" ? (
                    <CircleCheck className="size-4 text-success" aria-label="Fait" />
                  ) : state === "error" ? (
                    <CircleX className="size-4 text-destructive" aria-label="Échec" />
                  ) : (
                    <Checkbox
                      checked={!skipped.has(step.id)}
                      disabled={busy || finished}
                      aria-label={step.title}
                      onCheckedChange={(checked) =>
                        setSkipped((s) => {
                          const next = new Set(s);
                          if (checked) next.delete(step.id);
                          else next.add(step.id);
                          return next;
                        })
                      }
                    />
                  )}
                </span>
                <span className="min-w-0">
                  <span className="block text-sm font-medium">{step.title}</span>
                  <span className="block text-xs text-muted-foreground">{step.description}</span>
                </span>
              </li>
            );
          })}
        </ul>

        {running && <p className="text-sm text-warning">Ferme le jeu avant de réparer cette instance.</p>}

        <DialogFooter>
          <Button variant="outline" disabled={busy} onClick={() => onOpenChange(false)}>
            {finished ? "Fermer" : "Annuler"}
          </Button>
          {!finished && (
            <Button
              disabled={busy || running || isLoading || steps.every((s) => skipped.has(s.id))}
              onClick={() => repair.mutate()}
              className="gap-1.5"
            >
              {busy ? <Loader2 className="animate-spin" aria-hidden="true" /> : <Wrench aria-hidden="true" />}
              {busy ? "Réparation…" : "Réparer"}
            </Button>
          )}
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
