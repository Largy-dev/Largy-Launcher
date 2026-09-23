import { useNavigate } from "react-router";

import { useSettings } from "@/hooks/useSettings";
import { notify } from "@/lib/notify";
import { errorMessage, instancesApi, isCancelled, launchApi, type Instance } from "@/services/tauri";
import { runtimeOf, useAppStore } from "@/store/appStore";

/**
 * Play/stop for one instance plus its live state — shared by the hero, the
 * instance cards and the launch screen so they all behave the same way.
 */
export function useLaunchInstance(instance: Instance | undefined) {
  const navigate = useNavigate();
  const id = instance?.id ?? "";
  const account = useAppStore((s) => s.account);
  const runtime = useAppStore((s) => runtimeOf(s.runtime, id));
  const downloadProgress = useAppStore((s) => s.downloadProgress);
  const setRunning = useAppStore((s) => s.setRunning);
  const markStopping = useAppStore((s) => s.markStopping);
  const clearLogs = useAppStore((s) => s.clearLogs);
  const clearCrashAnalysis = useAppStore((s) => s.clearCrashAnalysis);
  const setActiveInstanceId = useAppStore((s) => s.setActiveInstanceId);
  const { data: settings } = useSettings();

  const installing =
    downloadProgress?.task_id === id &&
    downloadProgress.files_total > 0 &&
    downloadProgress.files_done < downloadProgress.files_total;
  const installPercent =
    installing && downloadProgress && downloadProgress.bytes_total > 0
      ? Math.round((downloadProgress.bytes_done / downloadProgress.bytes_total) * 100)
      : 0;

  const running = runtime.running;
  const preparing = running && runtime.phase !== "running";
  const canPlay = !!account || !!settings?.offline_mode;

  async function play() {
    if (!instance) return;
    if (!canPlay) {
      notify.warning({
        title: "Connexion requise",
        message: "Connecte-toi avec ton compte Microsoft, ou active le Mode Hors-ligne dans Paramètres › Compte.",
        action: { label: "Paramètres", onClick: () => navigate("/settings?tab=account") },
        history: false,
      });
      return;
    }
    setActiveInstanceId(instance.id);
    clearLogs(instance.id);
    clearCrashAnalysis(instance.id);
    setRunning(instance.id, true);
    navigate(`/instances/${instance.id}/launch`);
    try {
      await launchApi.launch(instance.id);
    } catch (e) {
      setRunning(instance.id, false);
      if (!isCancelled(e)) notify.error({ title: `Impossible de lancer ${instance.name}`, message: errorMessage(e) });
    }
  }

  async function stop() {
    if (!instance) return;
    markStopping(instance.id);
    try {
      await launchApi.stop(instance.id);
    } catch (e) {
      notify.error({ title: "Arrêt impossible", message: errorMessage(e), history: false });
    }
  }

  async function cancelInstall() {
    if (!instance) return;
    await instancesApi.cancelInstall(instance.id).catch(() => {});
  }

  return {
    play,
    stop,
    cancelInstall,
    running,
    preparing,
    phase: runtime.phase,
    startedAt: runtime.startedAt,
    installing,
    installPercent,
  };
}
