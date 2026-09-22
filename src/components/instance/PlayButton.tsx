import { motion } from "motion/react";
import { Loader2, Play, Square } from "lucide-react";

import { Button } from "@/components/ui/button";
import { useLaunchInstance } from "@/hooks/useLaunchInstance";
import { PHASE_META } from "@/lib/launchPhases";
import { cn } from "@/lib/utils";
import type { Instance } from "@/services/tauri";

interface PlayButtonProps {
  instance: Instance;
  size?: "sm" | "lg";
  className?: string;
}

/** Play → Préparation (current step) → En jeu + Arrêter, with the accent glow when idle. */
export function PlayButton({ instance, size = "sm", className }: PlayButtonProps) {
  const { play, stop, running, preparing, phase, installing, installPercent } = useLaunchInstance(instance);
  const large = size === "lg";

  if (installing) {
    return (
      <Button disabled size={large ? "lg" : "sm"} className={cn("gap-1.5", large && "h-12 px-6 text-base", className)}>
        <Loader2 className="animate-spin" aria-hidden="true" />
        Installation {installPercent}%
      </Button>
    );
  }

  if (running) {
    return (
      <div className={cn("flex items-center gap-2", className)}>
        {preparing ? (
          <span
            className={cn(
              "flex items-center gap-1.5 rounded-lg bg-primary/12 px-3 font-medium text-primary",
              large ? "h-12 text-base" : "h-7 text-xs",
            )}
          >
            <Loader2 className={cn("animate-spin", large ? "size-4" : "size-3.5")} aria-hidden="true" />
            {phase ? PHASE_META[phase].label : "Préparation"}…
          </span>
        ) : (
          <span
            className={cn(
              "flex items-center gap-1.5 rounded-lg bg-success/12 px-3 font-medium text-success",
              large ? "h-12 text-base" : "h-7 text-xs",
            )}
          >
            <span className="size-2 animate-pulse rounded-full bg-success" aria-hidden="true" />
            En jeu
          </span>
        )}
        <Button
          variant="destructive"
          size={large ? "lg" : "sm"}
          onClick={(e) => {
            e.stopPropagation();
            stop();
          }}
          className={cn("gap-1.5", large && "h-12 px-4")}
          title="Arrêter le jeu"
        >
          <Square className="fill-current" aria-hidden="true" />
          {large && "Arrêter"}
        </Button>
      </div>
    );
  }

  return (
    <motion.div whileHover={{ scale: 1.04 }} whileTap={{ scale: 0.97 }} className={cn("inline-flex", className)}>
      <Button
        onClick={(e) => {
          e.stopPropagation();
          play();
        }}
        size={large ? "lg" : "sm"}
        className={cn(
          "bg-gradient-brand shadow-glow gap-1.5 font-semibold hover:brightness-110",
          large && "h-12 gap-2 px-8 text-base",
        )}
      >
        <Play className={cn("fill-current", large && "size-5")} aria-hidden="true" />
        Jouer
      </Button>
    </motion.div>
  );
}
