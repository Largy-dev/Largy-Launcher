import { AnimatePresence, motion } from "motion/react";
import { CheckCheck, Power, PowerOff, Trash2, X } from "lucide-react";

import { Button } from "@/components/ui/button";
import type { InstalledItem } from "@/services/content";

interface SelectionBarProps {
  selected: InstalledItem[];
  busy: boolean;
  onSelectAll: () => void;
  onSetEnabled: (names: string[], enabled: boolean) => void;
  onDelete: (items: InstalledItem[]) => void;
  onClear: () => void;
}

/** Floating actions for the selected files. */
export function SelectionBar({ selected, busy, onSelectAll, onSetEnabled, onDelete, onClear }: SelectionBarProps) {
  // Unpacked folders can't be disabled: only files are switched on and off.
  const files = selected.filter((i) => !i.is_dir).map((i) => i.file_name);
  return (
    <AnimatePresence>
      {selected.length > 0 && (
        <motion.div
          initial={{ opacity: 0, y: 24 }}
          animate={{ opacity: 1, y: 0 }}
          exit={{ opacity: 0, y: 24 }}
          className="glass-strong fixed bottom-6 left-1/2 z-40 flex -translate-x-1/2 items-center gap-1 rounded-2xl p-1.5 pl-4 shadow-2xl ring-1 ring-border"
          role="toolbar"
          aria-label="Actions sur la sélection"
        >
          <span className="mr-2 text-sm font-semibold tabular-nums">
            {selected.length} sélectionné{selected.length > 1 ? "s" : ""}
          </span>
          <Button variant="ghost" size="sm" className="gap-1.5" onClick={onSelectAll}>
            <CheckCheck aria-hidden="true" />
            Tout
          </Button>
          {files.length > 0 && (
            <>
              <Button
                variant="ghost"
                size="sm"
                className="gap-1.5"
                disabled={busy}
                onClick={() => onSetEnabled(files, true)}
              >
                <Power aria-hidden="true" />
                Activer
              </Button>
              <Button
                variant="ghost"
                size="sm"
                className="gap-1.5"
                disabled={busy}
                onClick={() => onSetEnabled(files, false)}
              >
                <PowerOff aria-hidden="true" />
                Désactiver
              </Button>
            </>
          )}
          <Button
            variant="ghost"
            size="sm"
            className="gap-1.5 text-destructive hover:text-destructive"
            onClick={() => onDelete(selected)}
          >
            <Trash2 aria-hidden="true" />
            Supprimer
          </Button>
          <Button variant="ghost" size="icon-sm" aria-label="Annuler la sélection" onClick={onClear}>
            <X aria-hidden="true" />
          </Button>
        </motion.div>
      )}
    </AnimatePresence>
  );
}
