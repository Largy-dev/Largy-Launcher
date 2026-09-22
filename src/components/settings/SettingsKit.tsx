import type { ReactNode } from "react";
import { useBlocker } from "react-router";
import { AnimatePresence, motion } from "motion/react";
import { Loader2, type LucideIcon } from "lucide-react";

import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { spring } from "@/lib/motion";
import { cn } from "@/lib/utils";

export interface SettingsTab<T extends string> {
  id: T;
  label: string;
  icon: LucideIcon;
  description: string;
}

interface SettingsLayoutProps<T extends string> {
  tabs: SettingsTab<T>[];
  active: T;
  onChange: (tab: T) => void;
  children: ReactNode;
}

/** Vertical tab list on the left, animated content on the right. */
export function SettingsLayout<T extends string>({ tabs, active, onChange, children }: SettingsLayoutProps<T>) {
  const current = tabs.find((t) => t.id === active) ?? tabs[0];
  return (
    <div className="flex flex-1 gap-6">
      <nav
        aria-label="Sections"
        className="glass sticky top-0 flex h-fit w-52 shrink-0 flex-col gap-0.5 rounded-2xl p-2"
      >
        {tabs.map(({ id, label, icon: Icon }) => {
          const selected = id === active;
          return (
            <button
              key={id}
              onClick={() => onChange(id)}
              aria-current={selected ? "page" : undefined}
              className={cn(
                "relative flex items-center gap-2.5 rounded-lg px-3 py-2 text-left text-sm transition-colors",
                selected ? "font-semibold text-foreground" : "text-muted-foreground hover:text-foreground",
              )}
            >
              {selected && (
                <motion.span
                  layoutId="settings-tab"
                  transition={spring}
                  className="absolute inset-0 rounded-lg bg-accent"
                />
              )}
              <Icon className={cn("relative size-4", selected && "text-primary")} aria-hidden="true" />
              <span className="relative">{label}</span>
            </button>
          );
        })}
      </nav>
      <div className="min-w-0 flex-1 pb-24">
        <AnimatePresence mode="wait" initial={false}>
          <motion.div
            key={current.id}
            initial={{ opacity: 0, x: 12 }}
            animate={{ opacity: 1, x: 0 }}
            exit={{ opacity: 0, x: -12 }}
            transition={{ duration: 0.2 }}
          >
            <div className="mb-5">
              <h3 className="text-lg font-bold">{current.label}</h3>
              <p className="text-sm text-muted-foreground">{current.description}</p>
            </div>
            {children}
          </motion.div>
        </AnimatePresence>
      </div>
    </div>
  );
}

export function SettingSection({ title, children }: { title?: string; children: ReactNode }) {
  return (
    <section className="mb-5">
      {title && (
        <h4 className="mb-2 px-1 text-[0.7rem] font-semibold tracking-wider text-muted-foreground uppercase">
          {title}
        </h4>
      )}
      <div className="glass divide-y divide-border/60 rounded-2xl">{children}</div>
    </section>
  );
}

interface SettingRowProps {
  label: string;
  description?: ReactNode;
  control?: ReactNode;
  /** Put the control under the text instead of on the right (wide controls). */
  stacked?: boolean;
}

export function SettingRow({ label, description, control, stacked }: SettingRowProps) {
  return (
    <div className={cn("gap-6 px-4 py-3.5", stacked ? "space-y-3" : "flex items-center justify-between")}>
      <div className="min-w-0">
        <p className="text-sm font-medium">{label}</p>
        {description && <div className="text-xs text-muted-foreground">{description}</div>}
      </div>
      {control && <div className={cn(!stacked && "shrink-0")}>{control}</div>}
    </div>
  );
}

interface ChoiceOption<T extends string> {
  value: T;
  label: string;
  icon?: LucideIcon;
}

/** Segmented control for a handful of exclusive options. */
export function ChoiceGroup<T extends string>({
  value,
  options,
  onChange,
}: {
  value: T;
  options: ChoiceOption<T>[];
  onChange: (value: T) => void;
}) {
  return (
    <div role="radiogroup" className="flex rounded-lg bg-muted p-0.5">
      {options.map(({ value: v, label, icon: Icon }) => (
        <button
          key={v}
          role="radio"
          aria-checked={v === value}
          onClick={() => onChange(v)}
          className={cn(
            "flex items-center gap-1.5 rounded-md px-3 py-1.5 text-xs font-medium transition-all",
            v === value ? "bg-background text-foreground shadow-sm" : "text-muted-foreground hover:text-foreground",
          )}
        >
          {Icon && <Icon className="size-3.5" aria-hidden="true" />}
          {label}
        </button>
      ))}
    </div>
  );
}

interface UnsavedChangesProps {
  dirty: boolean;
  saving: boolean;
  onSave: () => Promise<unknown> | void;
  onReset: () => void;
}

/**
 * Floating "unsaved changes" bar, plus a guard dialog when leaving the page
 * with pending edits. Saving from the dialog continues the navigation.
 */
export function UnsavedChanges({ dirty, saving, onSave, onReset }: UnsavedChangesProps) {
  const blocker = useBlocker(
    ({ currentLocation, nextLocation }) => dirty && currentLocation.pathname !== nextLocation.pathname,
  );

  async function saveAndLeave() {
    try {
      await onSave();
      blocker.proceed?.();
    } catch {
      // onSave already reported the error — stay on the page.
    }
  }

  return (
    <>
      <AnimatePresence>
        {dirty && (
          <motion.div
            initial={{ opacity: 0, y: 40 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: 40 }}
            transition={spring}
            className="glass-strong fixed bottom-5 left-1/2 z-30 ml-30 flex -translate-x-1/2 items-center gap-4 rounded-2xl py-2.5 pr-2.5 pl-5 shadow-2xl"
          >
            <span className="size-2 animate-pulse rounded-full bg-warning" aria-hidden="true" />
            <p className="text-sm font-medium">Modifications non enregistrées</p>
            <div className="flex gap-2">
              <Button variant="ghost" size="sm" onClick={onReset}>
                Annuler
              </Button>
              <Button size="sm" onClick={() => onSave()} disabled={saving} className="bg-gradient-brand gap-1.5">
                {saving && <Loader2 className="animate-spin" aria-hidden="true" />}
                Enregistrer
              </Button>
            </div>
          </motion.div>
        )}
      </AnimatePresence>

      <Dialog open={blocker.state === "blocked"} onOpenChange={(open) => !open && blocker.reset?.()}>
        <DialogContent className="sm:max-w-lg">
          <DialogHeader>
            <DialogTitle>Modifications non enregistrées</DialogTitle>
            <DialogDescription>
              Tu as des changements non enregistrés. Les enregistrer avant de continuer ?
            </DialogDescription>
          </DialogHeader>
          <DialogFooter className="flex-wrap">
            <Button variant="outline" onClick={() => blocker.reset?.()}>
              Annuler
            </Button>
            <Button variant="outline" onClick={() => blocker.proceed?.()}>
              Ignorer les changements
            </Button>
            <Button onClick={saveAndLeave} disabled={saving} className="gap-1.5">
              {saving && <Loader2 className="size-3.5 animate-spin" aria-hidden="true" />}
              Enregistrer et continuer
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </>
  );
}
