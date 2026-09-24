import { useEffect, useRef } from "react";

import { notify } from "@/lib/notify";
import { useAppStore } from "@/store/appStore";
import { usePreferences } from "@/store/preferencesStore";

const CHECK_INTERVAL_MS = 15_000;

/** Nudges the player every `playtimeReminderMinutes` while a game is running, if enabled. */
export function usePlaytimeReminder() {
  const enabled = usePreferences((s) => s.notifications.playtimeReminder);
  const minutes = usePreferences((s) => s.playtimeReminderMinutes);
  const lastNotified = useRef<Record<string, number>>({});

  useEffect(() => {
    if (!enabled) return;
    const interval = setInterval(() => {
      for (const [instanceId, session] of Object.entries(useAppStore.getState().runtime)) {
        if (!session.running || session.startedAt === null) continue;
        const elapsedMinutes = (Date.now() - session.startedAt) / 60_000;
        const multiple = Math.floor(elapsedMinutes / minutes);
        if (multiple > 0 && lastNotified.current[instanceId] !== multiple) {
          lastNotified.current[instanceId] = multiple;
          notify.info({
            title: "Ça fait un moment que tu joues",
            message: `${multiple * minutes} minutes sur cette instance.`,
            native: "playtimeReminder",
          });
        }
      }
    }, CHECK_INTERVAL_MS);
    return () => clearInterval(interval);
  }, [enabled, minutes]);
}
