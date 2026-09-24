import { useNavigate } from "react-router";

import { useSettings } from "@/hooks/useSettings";
import { notify } from "@/lib/notify";
import { errorMessage, instancesApi, isAppError, isCancelled, launchApi, type Instance } from "@/services/tauri";
import { runtimeOf, useAppStore } from "@/store/appStore";

/**
 * Starts any instance (optionally straight onto a server): checks there is
 * an account to play with, then opens the launch screen.
 */
export function usePlayInstance() {
  const navigate = useNavigate();
  const account = useAppStore((s) => s.account);
  const setRunning = useAppStore((s) => s.setRunning);
  const clearLogs = useAppStore((s) => s.clearLogs);
  const clearCrashAnalysis = useAppStore((s) => s.clearCrashAnalysis);
  const setActiveInstanceId = useAppStore((s) => s.setActiveInstanceId);
  const { data: settings } = useSettings();
  const canPlay = !!account || !!settings?.offline_mode;
  // Offline mode left on with no name while a Microsoft account is connected:
  // almost always a mistake, so ask rather than play under a made-up name.
  const offlineMisconfigured = !!account && !!settings?.offline_mode && !settings.offline_username.trim();
  const openAccountSettings = { label: "Paramètres", onClick: () => navigate("/settings?tab=account") };

  return async function play(instance: Instance, server?: string) {
    if (offlineMisconfigured) {
      notify.warning({
        title: "Vérifie tes paramètres de compte",
        message:
          "Le mode hors-ligne est activé sans pseudo alors que ton compte Microsoft est connecté. Désactive-le pour jouer avec ton compte, ou choisis un pseudo.",
        action: openAccountSettings,
        history: false,
      });
      return;
    }
    if (!canPlay) {
      notify.warning({
        title: "Connexion requise",
        message: "Connecte-toi avec ton compte Microsoft, ou active le Mode Hors-ligne dans Paramètres › Compte.",
        action: openAccountSettings,
        history: false,
      });
      return;
    }
    if (runtimeOf(useAppStore.getState().runtime, instance.id).running) {
      navigate(`/instances/${instance.id}/launch`);
      return;
    }
    setActiveInstanceId(instance.id);
    clearLogs(instance.id);
    clearCrashAnalysis(instance.id);
    setRunning(instance.id, true);
    navigate(`/instances/${instance.id}/launch`);
    try {
      await launchApi.launch(instance.id, server);
    } catch (e) {
      setRunning(instance.id, false);
      if (!isCancelled(e)) {
        notify.error({
          title: `Impossible de lancer ${instance.name}`,
          message: errorMessage(e),
          action: isAppError(e) && e.kind === "auth" ? openAccountSettings : undefined,
        });
      }
    }
  };
}

/**
 * Play/stop for one instance plus its live state — shared by the hero, the
 * instance cards and the launch screen so they all behave the same way.
 */
export function useLaunchInstance(instance: Instance | undefined) {
  const id = instance?.id ?? "";
  const runtime = useAppStore((s) => runtimeOf(s.runtime, id));
  const downloadProgress = useAppStore((s) => s.downloadProgress);
  const markStopping = useAppStore((s) => s.markStopping);
  const playInstance = usePlayInstance();

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

  async function play(server?: string) {
    if (instance) await playInstance(instance, server);
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
