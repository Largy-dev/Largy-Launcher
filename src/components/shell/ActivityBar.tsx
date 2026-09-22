import { AnimatePresence, motion } from "motion/react";
import { Download } from "lucide-react";

import { useAppStore } from "@/store/appStore";

/** Slim global progress strip, visible from any page while something downloads. */
export function ActivityBar() {
  const progress = useAppStore((s) => s.downloadProgress);
  const active = !!progress && progress.files_total > 0 && progress.files_done < progress.files_total;
  const percent =
    active && progress.bytes_total > 0
      ? Math.min(100, Math.round((progress.bytes_done / progress.bytes_total) * 100))
      : 0;

  return (
    <AnimatePresence>
      {active && (
        <motion.div
          initial={{ opacity: 0, y: -8 }}
          animate={{ opacity: 1, y: 0 }}
          exit={{ opacity: 0, y: -8 }}
          className="glass-strong sticky top-0 z-20 -mx-6 -mt-6 mb-4 flex items-center gap-3 border-x-0 border-t-0 px-6 py-2 text-xs"
          role="status"
        >
          <Download className="size-3.5 shrink-0 animate-pulse text-primary" aria-hidden="true" />
          <span className="min-w-0 flex-1 truncate text-muted-foreground">
            {progress.label} · {progress.files_done}/{progress.files_total} fichiers
          </span>
          <div className="relative h-1.5 w-40 overflow-hidden rounded-full bg-muted">
            <motion.div
              className="bg-gradient-brand absolute inset-y-0 left-0 rounded-full"
              animate={{ width: `${percent}%` }}
              transition={{ ease: "easeOut" }}
            />
            <div className="shimmer-bg absolute inset-0 animate-shimmer" />
          </div>
          <span className="w-9 text-right font-medium tabular-nums">{percent}%</span>
        </motion.div>
      )}
    </AnimatePresence>
  );
}
