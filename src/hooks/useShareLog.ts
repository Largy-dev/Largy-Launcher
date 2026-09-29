import { useMutation } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";

import type { LogSource } from "@/bindings/LogSource";
import { notify } from "@/lib/notify";
import { errorMessage } from "@/services/tauri";

export interface ShareLogVars {
  source: LogSource;
  instanceId?: string;
  /** A specific crash report of the instance (path), else its newest. */
  crashReport?: string | null;
}

/** Uploads a log to mclo.gs (scrubbed of personal details) and copies the link. */
export function useShareLog() {
  return useMutation({
    mutationFn: (vars: ShareLogVars) =>
      invoke<string>("logs_share", {
        source: vars.source,
        instanceId: vars.instanceId ?? null,
        crashReport: vars.crashReport ?? null,
      }),
    onSuccess: async (url) => {
      let copied = true;
      try {
        await navigator.clipboard.writeText(url);
      } catch {
        copied = false;
      }
      notify.success({
        title: copied ? "Lien du log copié" : "Log partagé",
        message: `${url} — colle-le sur Discord pour demander de l'aide. Ton nom de session Windows et tes jetons ont été retirés.`,
        action: { label: "Ouvrir", onClick: () => openUrl(url) },
      });
    },
    onError: (e) => notify.error({ title: "Partage impossible", message: errorMessage(e) }),
  });
}
