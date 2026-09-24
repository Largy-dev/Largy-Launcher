import { useEffect, useRef, useState } from "react";

import { usePlayInstance } from "@/hooks/useLaunchInstance";
import { notify } from "@/lib/notify";
import { instancesApi, launchApi, onLaunchRequest } from "@/services/tauri";
import { useAppStore } from "@/store/appStore";

/**
 * Plays the instance a desktop shortcut points at — whether it started the
 * launcher or reached an already-open one. Waits for the startup login so
 * the account check doesn't fire too early.
 */
export function useLaunchRequests() {
  const authReady = useAppStore((s) => s.authReady);
  const playInstance = usePlayInstance();
  const [requested, setRequested] = useState<string | null>(null);
  const handling = useRef<string | null>(null);

  useEffect(() => {
    launchApi
      .takePendingLaunch()
      .then((id) => id && setRequested(id))
      .catch(() => {});
    const unlisten = onLaunchRequest(setRequested);
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  useEffect(() => {
    if (!requested || !authReady || handling.current === requested) return;
    handling.current = requested;
    instancesApi
      .get(requested)
      .then((instance) => playInstance(instance))
      .catch(() =>
        notify.error({
          title: "Instance introuvable",
          message: "Ce raccourci pointe vers une instance qui n'existe plus.",
          history: false,
        }),
      )
      .finally(() => {
        handling.current = null;
        setRequested(null);
      });
  }, [requested, authReady, playInstance]);
}
