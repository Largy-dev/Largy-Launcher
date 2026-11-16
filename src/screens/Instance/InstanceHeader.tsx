import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { motion } from "motion/react";
import { ChevronDown, FolderOpen, Puzzle, RefreshCw, Sparkles, Wrench } from "lucide-react";

import { InstanceIcon } from "@/components/instance/InstanceIcon";
import { LOADER_META, LoaderBadge } from "@/components/instance/LoaderBadge";
import { RepairDialog } from "@/components/instance/RepairDialog";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { useModpackUpdate } from "@/hooks/useInstances";
import { useNow, useRamAdvice } from "@/hooks/useInstanceInfo";
import { formatDuration, formatGb, formatRelative } from "@/lib/format";
import { fadeUp, stagger } from "@/lib/motion";
import { RAM_STATUS_CLASS } from "@/lib/ramAdvice";
import { cn } from "@/lib/utils";
import { ModpackDetailDialog } from "@/screens/ModpackBrowser/ModpackDetailDialog";
import { installedApi } from "@/services/content";
import { instancesApi, type Instance, type ModpackSummary, type ProviderId } from "@/services/tauri";
import { runtimeOf, useAppStore } from "@/store/appStore";

import { HeroPlayButton } from "./HeroPlayButton";
import { QuickJoin } from "./QuickJoin";

const PROVIDER_LABEL: Record<string, string> = {
  modrinth: "Modrinth",
  curseforge: "CurseForge",
  ftb: "FTB",
  url: "Pack partagé",
};

function Chip({ children, className }: { children: React.ReactNode; className?: string }) {
  return (
    <span
      className={cn(
        "inline-flex h-7 items-center gap-1.5 rounded-lg bg-foreground/8 px-2.5 text-xs font-medium backdrop-blur",
        className,
      )}
    >
      {children}
    </span>
  );
}

/** The modpack as the detail dialog expects it, rebuilt from what the instance remembers. */
function packSummaryOf(instance: Instance): ModpackSummary | null {
  const pack = instance.modpack;
  if (!pack) return null;
  return {
    id: pack.pack_id,
    provider: pack.provider,
    name: pack.pack_name,
    author: "",
    icon_url: instance.icon_url,
    summary: "",
    downloads: null,
    game_versions: [instance.minecraft_version],
    loaders: [instance.loader],
    updated_at: null,
  };
}

/** "Tu as joué 12 h ! · Dernière partie : hier · 143 mods actifs · 6 Go de RAM" */
function StatsLine({ instance }: { instance: Instance }) {
  const runtime = useAppStore((s) => runtimeOf(s.runtime, instance.id));
  const now = useNow(30_000, runtime.running);
  const { data: summary } = useQuery({
    queryKey: ["content-summary", instance.id],
    queryFn: () => installedApi.summary(instance.id),
    staleTime: 30_000,
  });
  const mods = instance.loader === "vanilla" ? null : (summary?.mods_enabled ?? null);
  const ram = useRamAdvice(instance, mods);

  const parts: React.ReactNode[] = [];
  if (runtime.running && runtime.startedAt) {
    parts.push(`En jeu depuis ${formatDuration((now - runtime.startedAt) / 1000)}`);
  } else if (instance.play_time_seconds > 0) {
    parts.push(`Tu as joué ${formatDuration(instance.play_time_seconds)} !`);
  } else {
    parts.push("Jamais lancée — c'est le moment !");
  }
  if (instance.last_played_at && !runtime.running) {
    parts.push(`Dernière partie : ${formatRelative(instance.last_played_at)}`);
  }
  if (mods !== null) parts.push(`${mods} mod${mods > 1 ? "s" : ""} actif${mods > 1 ? "s" : ""}`);
  if (summary && summary.worlds > 0) parts.push(`${summary.worlds} monde${summary.worlds > 1 ? "s" : ""}`);
  if (ram) {
    parts.push(
      <span key="ram" className="inline-flex items-center gap-1.5" title={ram.message}>
        {formatGb(ram.allocated)} de RAM
        <span className={cn("size-1.5 rounded-full", RAM_STATUS_CLASS[ram.status])} aria-hidden="true" />
      </span>,
    );
  }
  if (instance.auto_join_server) parts.push(`Rejoint ${instance.auto_join_server}`);

  return (
    <p className="flex flex-wrap items-center gap-x-1.5 text-sm text-foreground/80">
      {parts.map((part, i) => (
        <span key={i} className="inline-flex items-center gap-1.5">
          {i > 0 && <span className="text-foreground/35">·</span>}
          {part}
        </span>
      ))}
    </p>
  );
}

function VersionBox({ instance, onChangePack }: { instance: Instance; onChangePack: () => void }) {
  const update = useModpackUpdate(instance);
  const loader = LOADER_META[instance.loader].label;
  const value =
    instance.loader === "vanilla"
      ? `Minecraft ${instance.minecraft_version}`
      : `${loader} ${instance.loader_version ?? ""} · ${instance.minecraft_version}`.replace("  ", " ");

  const body = (
    <>
      <span className="min-w-0 text-left">
        <span className="block text-[0.68rem] text-muted-foreground">
          {instance.modpack ? "Version du pack" : "Version"}
        </span>
        <span className="block max-w-56 truncate text-sm font-semibold">{value}</span>
      </span>
      {instance.modpack && <ChevronDown className="size-4 shrink-0 text-muted-foreground" aria-hidden="true" />}
      {update && <span className="absolute top-2 right-2 size-2 rounded-full bg-info" aria-hidden="true" />}
    </>
  );

  if (!instance.modpack) {
    return <div className="tile relative flex h-14 items-center gap-3 px-4">{body}</div>;
  }
  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <button className="tile relative flex h-14 items-center gap-3 px-4 transition-colors hover:bg-[var(--surface-hover)]">
          {body}
        </button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="start" className="w-64">
        {update && (
          <DropdownMenuItem className="gap-2 text-info" onClick={onChangePack}>
            <RefreshCw className="size-4" aria-hidden="true" />
            <span className="min-w-0">
              Mettre à jour
              <span className="block truncate text-xs text-muted-foreground">{update.name}</span>
            </span>
          </DropdownMenuItem>
        )}
        <DropdownMenuItem className="gap-2" onClick={onChangePack}>
          <Sparkles className="size-4" aria-hidden="true" />
          Changer de version du pack…
        </DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}

function IconTile({ icon: Icon, label, onClick }: { icon: typeof Puzzle; label: string; onClick: () => void }) {
  return (
    <button
      onClick={onClick}
      title={label}
      aria-label={label}
      className="tile flex size-14 shrink-0 items-center justify-center text-foreground/80 transition-colors hover:bg-[var(--surface-hover)] hover:text-foreground"
    >
      <Icon className="size-5" aria-hidden="true" />
    </button>
  );
}

interface InstanceHeaderProps {
  instance: Instance;
  onBrowseMods: () => void;
}

/** Big title block of the instance page: chips, name, stats, then Jouer and its neighbours. */
export function InstanceHeader({ instance, onBrowseMods }: InstanceHeaderProps) {
  const [changingPack, setChangingPack] = useState(false);
  const [repairing, setRepairing] = useState(false);
  const running = useAppStore((s) => runtimeOf(s.runtime, instance.id).running);
  const update = useModpackUpdate(instance);

  return (
    <motion.section variants={stagger} initial="hidden" animate="show" className="pt-8 pb-6">
      <motion.div variants={fadeUp} className="mb-4 flex flex-wrap items-center gap-2">
        <LoaderBadge loader={instance.loader} version={instance.loader_version} className="h-7 rounded-lg px-2.5" />
        <Chip>Minecraft {instance.minecraft_version}</Chip>
        {instance.modpack && <Chip>{PROVIDER_LABEL[instance.modpack.provider] ?? instance.modpack.provider}</Chip>}
        {update && (
          <button onClick={() => setChangingPack(true)} disabled={running}>
            <Chip className="bg-info/15 text-info hover:bg-info/25">
              <RefreshCw className="size-3" aria-hidden="true" />
              Mise à jour disponible
            </Chip>
          </button>
        )}
      </motion.div>

      <motion.div variants={fadeUp} className="flex items-center gap-5">
        <InstanceIcon instance={instance} className="size-16 rounded-2xl shadow-2xl ring-1 ring-white/10" />
        <h1 className="min-w-0 truncate text-5xl leading-tight font-black tracking-tight drop-shadow-sm">
          {instance.name}
        </h1>
      </motion.div>

      <motion.div variants={fadeUp} className="mt-3">
        <StatsLine instance={instance} />
      </motion.div>

      <motion.div variants={fadeUp} className="mt-6 flex flex-wrap items-center gap-3">
        <HeroPlayButton instance={instance} />
        <VersionBox instance={instance} onChangePack={() => setChangingPack(true)} />
        {instance.loader !== "vanilla" && <IconTile icon={Puzzle} label="Ajouter des mods" onClick={onBrowseMods} />}
        <IconTile icon={FolderOpen} label="Ouvrir le dossier" onClick={() => instancesApi.openFolder(instance.id)} />
        <IconTile icon={Wrench} label="Réparer" onClick={() => setRepairing(true)} />
        <QuickJoin instance={instance} />
      </motion.div>

      <RepairDialog instance={instance} open={repairing} onOpenChange={setRepairing} />
      {changingPack && instance.modpack && (
        <ModpackDetailDialog
          provider={instance.modpack.provider as ProviderId}
          pack={packSummaryOf(instance)}
          onOpenChange={(open) => !open && setChangingPack(false)}
          updateInstanceId={instance.id}
        />
      )}
    </motion.section>
  );
}
