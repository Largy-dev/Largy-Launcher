import { useState } from "react";
import { AnimatePresence, motion } from "motion/react";
import { Loader2, X } from "lucide-react";

import { Button } from "@/components/ui/button";
import { installAppUpdate } from "@/lib/updater";
import { notify } from "@/lib/notify";
import { errorMessage } from "@/services/tauri";
import { useAppStore } from "@/store/appStore";

/** "Une nouvelle version est disponible" — stays in the sidebar until installed or dismissed. */
export function UpdateCard() {
  const update = useAppStore((s) => s.appUpdate);
  const [dismissed, setDismissed] = useState(false);
  const [percent, setPercent] = useState<number | null>(null);

  async function install() {
    if (!update) return;
    setPercent(0);
    try {
      await installAppUpdate(update, setPercent);
    } catch (e) {
      setPercent(null);
      notify.error({ title: "Mise à jour impossible", message: errorMessage(e) });
    }
  }

  return (
    <AnimatePresence>
      {update && !dismissed && (
        <motion.div
          initial={{ opacity: 0, y: 8 }}
          animate={{ opacity: 1, y: 0 }}
          exit={{ opacity: 0, y: 8 }}
          className="relative overflow-hidden rounded-xl border border-sidebar-border bg-sidebar-accent p-3"
        >
          <div
            aria-hidden="true"
            className="absolute inset-0 opacity-60"
            style={{
              backgroundImage:
                "radial-gradient(16rem 8rem at 100% 0%, color-mix(in oklab, var(--accent-base) 30%, transparent), transparent 70%)",
            }}
          />
          {percent === null && (
            <button
              onClick={() => setDismissed(true)}
              aria-label="Masquer"
              className="absolute top-2 right-2 rounded-md p-0.5 text-sidebar-foreground/40 hover:text-sidebar-foreground"
            >
              <X className="size-3.5" aria-hidden="true" />
            </button>
          )}
          <div className="relative">
            <p className="pr-4 text-sm leading-snug font-semibold">Une nouvelle version est disponible</p>
            <p className="mt-0.5 text-xs text-sidebar-foreground/55">
              v{update.currentVersion} → v{update.version}
            </p>
            <Button
              size="sm"
              disabled={percent !== null}
              onClick={install}
              className="mt-2.5 w-full gap-1.5 bg-foreground text-background hover:bg-foreground/85"
            >
              {percent !== null && <Loader2 className="animate-spin" aria-hidden="true" />}
              {percent === null ? "Mettre à jour" : `Téléchargement… ${percent}%`}
            </Button>
          </div>
        </motion.div>
      )}
    </AnimatePresence>
  );
}
