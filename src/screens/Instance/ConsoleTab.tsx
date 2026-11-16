import { useState } from "react";
import { AnimatePresence } from "motion/react";

import { LogConsole } from "@/components/console/LogConsole";
import { CrashCard, LiveStatsPanel, TransferPanel } from "@/components/launch/LaunchPanels";
import { LaunchTimeline } from "@/components/launch/LaunchTimeline";
import { ReportIssueDialog } from "@/components/ReportIssueDialog";
import { useAllocatedRam } from "@/hooks/useInstanceInfo";
import { useLaunchInstance } from "@/hooks/useLaunchInstance";
import { useShareLog } from "@/hooks/useShareLog";
import { instancesApi, type Instance } from "@/services/tauri";
import { runtimeOf, useAppStore } from "@/store/appStore";

/** Launch steps while starting, live stats while playing, the crash diagnosis after — above the game log. */
export function ConsoleTab({ instance }: { instance: Instance }) {
  const runtime = useAppStore((s) => runtimeOf(s.runtime, instance.id));
  const crashAnalysis = useAppStore((s) => s.crashAnalysis[instance.id]);
  const clearCrashAnalysis = useAppStore((s) => s.clearCrashAnalysis);
  const { play, running, preparing } = useLaunchInstance(instance);
  const allocated = useAllocatedRam(instance);
  const share = useShareLog();
  const [reporting, setReporting] = useState(false);

  const openCrashDetails = () => {
    const report = crashAnalysis?.crash_report;
    return report ? instancesApi.revealFile(instance.id, report) : instancesApi.openFolder(instance.id, "logs");
  };

  return (
    <div className="flex flex-col gap-4">
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
            onOpenFolder={openCrashDetails}
            onRelaunch={() => play()}
            sharing={share.isPending}
            onReport={() => setReporting(true)}
            onShare={() =>
              share.mutate(
                crashAnalysis.crash_report
                  ? { source: "crash", instanceId: instance.id, crashReport: crashAnalysis.crash_report }
                  : { source: "game", instanceId: instance.id },
              )
            }
          />
        )}
      </AnimatePresence>

      <LogConsole
        lines={runtime.logs}
        onOpenFolder={() => instancesApi.openFolder(instance.id, "logs")}
        onShare={() => share.mutate({ source: "game", instanceId: instance.id })}
        sharing={share.isPending}
        className="h-[28rem]"
      />
      <ReportIssueDialog
        open={reporting}
        onOpenChange={setReporting}
        instance={instance}
        crashSummary={crashAnalysis?.summary}
        crashReport={crashAnalysis?.crash_report}
      />
    </div>
  );
}
