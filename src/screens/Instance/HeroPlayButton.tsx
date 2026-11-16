import { Loader2, MoreHorizontal, Play, Square, X } from "lucide-react";

import { useInstanceMenu } from "@/components/instance/InstanceMenu";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { useNow } from "@/hooks/useInstanceInfo";
import { useLaunchInstance } from "@/hooks/useLaunchInstance";
import { formatClock } from "@/lib/format";
import { PHASE_META } from "@/lib/launchPhases";
import { cn } from "@/lib/utils";
import type { Instance } from "@/services/tauri";

/**
 * The big "Jouer" pill with its ⋯ menu glued on the right. While the game
 * starts or runs, the same pill shows the step / session clock and stops it.
 */
export function HeroPlayButton({ instance }: { instance: Instance }) {
  const { play, stop, cancelInstall, running, preparing, phase, startedAt, installing, installPercent } =
    useLaunchInstance(instance);
  const now = useNow(1000, running && !preparing && !!startedAt);
  const menu = useInstanceMenu(instance);

  const busy = running || installing;
  const mainClass = "flex items-center gap-2.5 pr-5 pl-6 text-lg font-semibold transition-colors";

  return (
    <div
      className={cn(
        "flex h-14 items-stretch overflow-hidden rounded-xl! transition-shadow",
        busy ? "panel" : "bg-gradient-brand shadow-glow text-primary-foreground",
      )}
    >
      {installing ? (
        <>
          <span className={cn(mainClass, "text-base")}>
            <Loader2 className="size-5 animate-spin text-primary" aria-hidden="true" />
            Installation {installPercent}%
          </span>
          <button
            onClick={cancelInstall}
            title="Annuler l'installation"
            aria-label="Annuler l'installation"
            className="px-4 text-muted-foreground transition-colors hover:bg-destructive/15 hover:text-destructive"
          >
            <X className="size-5" aria-hidden="true" />
          </button>
        </>
      ) : running ? (
        <>
          <span className={cn(mainClass, "text-base", preparing ? "text-primary" : "text-success")}>
            {preparing ? (
              <Loader2 className="size-5 animate-spin" aria-hidden="true" />
            ) : (
              <span className="size-2.5 animate-pulse rounded-full bg-success" aria-hidden="true" />
            )}
            {preparing
              ? `${phase ? PHASE_META[phase].label : "Préparation"}…`
              : `En jeu${startedAt ? ` · ${formatClock((now - startedAt) / 1000)}` : ""}`}
          </span>
          <button
            onClick={stop}
            title={preparing ? "Annuler le lancement" : "Arrêter le jeu"}
            className="flex items-center gap-1.5 bg-destructive/12 px-4 text-sm font-semibold text-destructive transition-colors hover:bg-destructive/22"
          >
            {preparing ? (
              <X className="size-4" aria-hidden="true" />
            ) : (
              <Square className="size-3.5 fill-current" aria-hidden="true" />
            )}
            {preparing ? "Annuler" : "Arrêter"}
          </button>
        </>
      ) : (
        <button onClick={() => play()} className={cn(mainClass, "hover:bg-white/10")}>
          <Play className="size-5" aria-hidden="true" />
          Jouer
        </button>
      )}
      <span className={cn("my-3.5 w-px", busy ? "bg-border" : "bg-white/25")} aria-hidden="true" />
      <DropdownMenu>
        <DropdownMenuTrigger asChild>
          <button
            title="Plus d'actions"
            aria-label="Plus d'actions"
            className={cn("px-3.5 transition-colors", busy ? "hover:bg-[var(--surface-hover)]" : "hover:bg-white/10")}
          >
            <MoreHorizontal className="size-5" aria-hidden="true" />
          </button>
        </DropdownMenuTrigger>
        <DropdownMenuContent align="start" className="w-60">
          {menu.items({ Item: DropdownMenuItem, Separator: DropdownMenuSeparator })}
        </DropdownMenuContent>
      </DropdownMenu>
      {menu.dialogs}
    </div>
  );
}
