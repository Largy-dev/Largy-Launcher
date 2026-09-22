import { motion } from "motion/react";
import { Check } from "lucide-react";

import { PHASE_META, phasesFor } from "@/lib/launchPhases";
import { cn } from "@/lib/utils";
import type { LaunchPhase, LoaderKind } from "@/services/tauri";

interface LaunchTimelineProps {
  loader: LoaderKind;
  phase: LaunchPhase | null;
}

/** Horizontal stepper: done steps checked, current one pulsing with its hint below. */
export function LaunchTimeline({ loader, phase }: LaunchTimelineProps) {
  const phases = phasesFor(loader);
  const current = phase ? phases.indexOf(phase) : -1;
  const active = phase ?? phases[0];

  return (
    <div className="glass rounded-2xl p-5">
      <ol className="flex items-start">
        {phases.map((p, i) => {
          const meta = PHASE_META[p];
          const Icon = meta.icon;
          const done = i < current;
          const isCurrent = i === current || (current === -1 && i === 0);
          return (
            <li key={p} className="flex flex-1 flex-col gap-2 last:flex-none">
              <div className="flex w-full items-center">
                <div className="relative">
                  {isCurrent && (
                    <motion.span
                      className="absolute inset-0 rounded-full bg-primary"
                      animate={{ scale: [1, 1.6], opacity: [0.5, 0] }}
                      transition={{ duration: 1.4, repeat: Infinity, ease: "easeOut" }}
                      aria-hidden="true"
                    />
                  )}
                  <div
                    className={cn(
                      "relative flex size-9 items-center justify-center rounded-full border-2 transition-colors",
                      done && "border-primary bg-primary text-primary-foreground",
                      isCurrent && "bg-gradient-brand border-transparent text-primary-foreground shadow-glow",
                      !done && !isCurrent && "border-border bg-muted text-muted-foreground",
                    )}
                  >
                    {done ? (
                      <Check className="size-4" aria-hidden="true" />
                    ) : (
                      <Icon className="size-4" aria-hidden="true" />
                    )}
                  </div>
                </div>
                {i < phases.length - 1 && (
                  <div className="relative mx-1 h-0.5 flex-1 overflow-hidden rounded-full bg-border">
                    <motion.div
                      className="bg-gradient-brand absolute inset-y-0 left-0"
                      initial={false}
                      animate={{ width: done ? "100%" : "0%" }}
                      transition={{ duration: 0.4 }}
                    />
                  </div>
                )}
              </div>
              <span
                className={cn(
                  "flex w-9 justify-center self-start text-[0.7rem] font-medium whitespace-nowrap",
                  isCurrent ? "text-foreground" : "text-muted-foreground",
                )}
              >
                {meta.label}
              </span>
            </li>
          );
        })}
      </ol>
      <p className="mt-4 text-center text-sm text-muted-foreground">{PHASE_META[active].hint}…</p>
    </div>
  );
}
