import { useEffect } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { useNavigate } from "react-router";
import { toast } from "sonner";

import { formatDuration } from "@/lib/format";
import { notify } from "@/lib/notify";
import { checkForAppUpdate, installAppUpdate } from "@/lib/updater";
import {
  auth,
  errorMessage,
  instancesApi,
  onDownloadProgress,
  onInstanceExit,
  onInstanceLog,
  onInstancesChanged,
  onLaunchPhase,
  type Instance,
} from "@/services/tauri";
import { runtimeOf, useAppStore } from "@/store/appStore";

/** Wires backend events and startup checks into the stores and notifications. Mounted once by the shell. */
export function useAppEvents() {
  const navigate = useNavigate();
  const queryClient = useQueryClient();

  useEffect(() => {
    const { setAccount } = useAppStore.getState();
    auth
      .trySilentLogin()
      .then((account) => {
        setAccount(account);
        if (account?.offline) {
          notify.warning({
            title: "Mode hors connexion",
            message: `Impossible de joindre Microsoft : ${account.profile.name} peut jouer en solo, pas en multijoueur.`,
            history: false,
          });
        }
      })
      .catch((e) => {
        setAccount(null);
        notify.warning({ title: "Reconnexion nécessaire", message: errorMessage(e), history: false });
      });
  }, []);

  useEffect(() => {
    checkForAppUpdate()
      .then((update) => {
        if (!update) return;
        notify.info({
          title: "Nouvelle version disponible",
          message: `v${update.currentVersion} → v${update.version}`,
          native: "updates",
          sticky: true,
          action: {
            label: "Mettre à jour",
            onClick: () => {
              const id = toast.loading("Téléchargement de la mise à jour…");
              installAppUpdate(update, (percent) => toast.loading(`Téléchargement… ${percent}%`, { id })).catch((e) =>
                toast.error(errorMessage(e), { id }),
              );
            },
          },
        });
      })
      .catch(() => {
        // Pas de connexion, GitHub indisponible, etc. — on ne bloque jamais le démarrage pour ça.
      });
  }, []);

  useEffect(() => {
    const store = useAppStore.getState;
    const instanceName = (id: string) =>
      queryClient.getQueryData<Instance[]>(["instances"])?.find((i) => i.id === id)?.name ?? "L'instance";

    const unlisten = [
      onDownloadProgress((p) => store().setDownloadProgress(p)),
      onInstanceLog((batch) => store().appendLogs(batch.instance_id, batch.lines)),
      onLaunchPhase((e) => store().setPhase(e.instance_id, e.phase)),
      onInstancesChanged(() => queryClient.invalidateQueries({ queryKey: ["instances"] })),
      onInstanceExit((e) => {
        const runtime = runtimeOf(store().runtime, e.instance_id);
        const name = instanceName(e.instance_id);
        const crash = e.killed ? null : e.crash_analysis;
        const session = runtime.startedAt ? (Date.now() - runtime.startedAt) / 1000 : 0;

        store().setRunning(e.instance_id, false);
        store().setCrashAnalysis(e.instance_id, crash);
        queryClient.invalidateQueries({ queryKey: ["instances"] });
        queryClient.invalidateQueries({ queryKey: ["instance", e.instance_id] });

        if (crash) {
          const report = crash.crash_report;
          notify.error({
            title: `${name} a crashé`,
            message: crash.suggestion ? `${crash.summary} ${crash.suggestion}` : crash.summary,
            native: "crash",
            sticky: true,
            action: report
              ? { label: "Crash report", onClick: () => instancesApi.revealFile(e.instance_id, report) }
              : { label: "Voir les logs", onClick: () => navigate(`/instances/${e.instance_id}/launch`) },
          });
        } else if (session > 0) {
          notify.info({
            title: `${name} fermé`,
            message: `Session de ${formatDuration(session)}. Bon retour !`,
            native: "gameExit",
          });
        }
      }),
    ];
    return () => {
      unlisten.forEach((p) => p.then((fn) => fn()));
    };
  }, [navigate, queryClient]);
}
