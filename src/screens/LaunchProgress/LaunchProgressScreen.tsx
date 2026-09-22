import { useQuery } from "@tanstack/react-query";
import { useNavigate, useParams } from "react-router";
import { AnimatePresence } from "motion/react";
import { ArrowLeft, Loader2 } from "lucide-react";

import { InstanceIcon } from "@/components/instance/InstanceIcon";
import { LoaderBadge } from "@/components/instance/LoaderBadge";
import { PlayButton } from "@/components/instance/PlayButton";
import { CrashCard, LiveStatsPanel, TransferPanel } from "@/components/launch/LaunchPanels";
import { LaunchTimeline } from "@/components/launch/LaunchTimeline";
import { LogConsole } from "@/components/console/LogConsole";
import { PageHeader } from "@/components/PageHeader";
import { Button } from "@/components/ui/button";
import { useAllocatedRam } from "@/hooks/useInstanceInfo";
import { useLaunchInstance } from "@/hooks/useLaunchInstance";
import { instancesApi } from "@/services/tauri";
import { runtimeOf, useAppStore } from "@/store/appStore";

export function LaunchProgressScreen() {
  const { id } = useParams<{ id: string }>();
  const instanceId = id ?? "";
  const navigate = useNavigate();
  const { data: instance } = useQuery({
    queryKey: ["instance", instanceId],
    queryFn: () => instancesApi.get(instanceId),
    enabled: instanceId !== "",
  });
  const runtime = useAppStore((s) => runtimeOf(s.runtime, instanceId));
  const crashAnalysis = useAppStore((s) => s.crashAnalysis[instanceId]);
  const clearCrashAnalysis = useAppStore((s) => s.clearCrashAnalysis);
  const { play, running, preparing } = useLaunchInstance(instance);
  const allocated = useAllocatedRam(instance);

  if (!instance) {
    return (
      <div className="flex flex-1 items-center justify-center">
        <Loader2 className="size-5 animate-spin text-muted-foreground" aria-hidden="true" />
      </div>
    );
  }

  const openFolder = () => instancesApi.openFolder(instance.id);
  const status = preparing
    ? "Lancement en cours"
    : running
      ? "En jeu"
      : crashAnalysis
        ? "Crash"
        : runtime.logs.length > 0
          ? "Session terminée"
          : "Journal";

  return (
    <div className="flex min-h-0 flex-1 flex-col gap-4">
      <PageHeader
        eyebrow={status}
        title={
          <span className="flex items-center gap-3">
            <InstanceIcon instance={instance} className="size-10 rounded-xl" />
            <span className="truncate">{instance.name}</span>
          </span>
        }
        description={
          <span className="flex items-center gap-2">
            <LoaderBadge loader={instance.loader} version={instance.loader_version} />
            Minecraft {instance.minecraft_version}
          </span>
        }
        action={
          <>
            <Button variant="ghost" size="sm" className="gap-1.5" onClick={() => navigate("/")}>
              <ArrowLeft aria-hidden="true" />
              Accueil
            </Button>
            <PlayButton instance={instance} size="lg" />
          </>
        }
      />

      <AnimatePresence>
        {preparing && <LaunchTimeline key="timeline" loader={instance.loader} phase={runtime.phase} />}
        {preparing && <TransferPanel key="transfer" />}
        {running && !preparing && <LiveStatsPanel key="stats" instanceId={instance.id} allocatedMb={allocated} />}
        {crashAnalysis && !running && (
          <CrashCard
            key="crash"
            analysis={crashAnalysis}
            logs={runtime.logs}
            onDismiss={() => clearCrashAnalysis(instance.id)}
            onOpenFolder={openFolder}
            onRelaunch={play}
          />
        )}
      </AnimatePresence>

      <LogConsole lines={runtime.logs} onOpenFolder={openFolder} className="min-h-72 flex-1" />
    </div>
  );
}
