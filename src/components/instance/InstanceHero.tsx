import type { ReactNode } from "react";
import { motion } from "motion/react";
import { useNavigate } from "react-router";
import { History, MemoryStick, Package, Settings2, Timer } from "lucide-react";

import { Button } from "@/components/ui/button";
import { useModCount, useRamAdvice } from "@/hooks/useInstanceInfo";
import { formatDuration, formatGb, formatRelative } from "@/lib/format";
import { fadeUp, stagger } from "@/lib/motion";
import { RAM_STATUS_CLASS } from "@/lib/ramAdvice";
import { cn } from "@/lib/utils";
import type { Instance } from "@/services/tauri";
import { useAppStore } from "@/store/appStore";

import { InstanceIcon } from "./InstanceIcon";
import { LOADER_META, LoaderBadge } from "./LoaderBadge";
import { PlayButton } from "./PlayButton";

function Stat({ icon: Icon, children, title }: { icon: typeof Timer; children: ReactNode; title?: string }) {
  return (
    <span title={title} className="flex items-center gap-1.5 text-sm text-foreground/75">
      <Icon className="size-4 text-foreground/45" aria-hidden="true" />
      {children}
    </span>
  );
}

/** Big banner for the featured instance on the home screen. */
export function InstanceHero({ instance }: { instance: Instance }) {
  const navigate = useNavigate();
  const running = useAppStore((s) => s.runtime[instance.id]?.running ?? false);
  const modCount = useModCount(instance);
  const ram = useRamAdvice(instance, modCount);
  const color = LOADER_META[instance.loader].color;

  return (
    <motion.section
      variants={stagger}
      initial="hidden"
      animate="show"
      className="glass relative mb-6 overflow-hidden rounded-2xl"
    >
      {instance.icon_url && (
        <img
          src={instance.icon_url}
          alt=""
          aria-hidden="true"
          className="absolute -top-1/2 right-0 h-[200%] w-2/3 object-cover opacity-30 blur-2xl saturate-150"
        />
      )}
      <div
        aria-hidden="true"
        className="absolute inset-0"
        style={{
          backgroundImage: `linear-gradient(100deg, color-mix(in oklab, ${color} 22%, transparent) 0%, transparent 55%), radial-gradient(40rem 20rem at 100% 0%, color-mix(in oklab, var(--accent-base) 20%, transparent), transparent 70%)`,
        }}
      />

      <div className="relative flex items-center gap-6 p-6">
        <motion.div variants={fadeUp} className="relative">
          <div
            className="absolute inset-2 rounded-2xl opacity-60 blur-xl"
            style={{ backgroundColor: color }}
            aria-hidden="true"
          />
          <InstanceIcon instance={instance} className="relative size-24 rounded-2xl shadow-2xl" />
        </motion.div>

        <div className="min-w-0 flex-1 space-y-2.5">
          <motion.p variants={fadeUp} className="text-xs font-semibold tracking-wider text-primary uppercase">
            {running ? "En cours" : instance.last_played_at ? "Reprendre" : "Prête à jouer"}
          </motion.p>
          <motion.h2 variants={fadeUp} className="truncate text-3xl font-black tracking-tight">
            {instance.name}
          </motion.h2>
          <motion.div variants={fadeUp} className="flex flex-wrap items-center gap-2">
            <LoaderBadge loader={instance.loader} version={instance.loader_version} />
            <span className="rounded-full bg-foreground/8 px-2 py-0.5 text-[0.7rem] font-semibold">
              Minecraft {instance.minecraft_version}
            </span>
            {instance.modpack && (
              <span className="rounded-full bg-foreground/8 px-2 py-0.5 text-[0.7rem] font-semibold">
                Modpack {instance.modpack.provider.toUpperCase()}
              </span>
            )}
          </motion.div>
          <motion.div variants={fadeUp} className="flex flex-wrap items-center gap-x-5 gap-y-1.5 pt-1">
            <Stat icon={Timer} title="Temps de jeu total">
              {instance.play_time_seconds > 0 ? formatDuration(instance.play_time_seconds) : "Jamais joué"}
            </Stat>
            {instance.last_played_at && <Stat icon={History}>{formatRelative(instance.last_played_at)}</Stat>}
            {modCount !== null && (
              <Stat icon={Package}>
                {modCount} mod{modCount > 1 ? "s" : ""}
              </Stat>
            )}
            {ram && (
              <Stat icon={MemoryStick} title={`${ram.message} Conseillé : ${formatGb(ram.advice.recommendedMb)}.`}>
                {formatGb(ram.allocated)}
                <span className={cn("size-2 rounded-full", RAM_STATUS_CLASS[ram.status])} aria-hidden="true" />
              </Stat>
            )}
          </motion.div>
        </div>

        <motion.div variants={fadeUp} className="flex shrink-0 flex-col items-end gap-2">
          <PlayButton instance={instance} size="lg" />
          <Button
            variant="ghost"
            size="sm"
            className="gap-1.5 text-muted-foreground"
            onClick={() => navigate(`/instances/${instance.id}`)}
          >
            <Settings2 aria-hidden="true" />
            Réglages
          </Button>
        </motion.div>
      </div>
    </motion.section>
  );
}
