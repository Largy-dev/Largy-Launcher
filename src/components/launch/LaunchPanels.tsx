import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { motion } from "motion/react";
import { AlertTriangle, Copy, Cpu, FolderOpen, MemoryStick, RotateCcw, Timer, X } from "lucide-react";

import { Button } from "@/components/ui/button";
import { useNow } from "@/hooks/useInstanceInfo";
import { formatBytes, formatClock, formatEta, formatGb, formatSpeed } from "@/lib/format";
import { notify } from "@/lib/notify";
import type { RawLogLine } from "@/lib/logParse";
import { cn } from "@/lib/utils";
import { launchApi, type CrashAnalysis } from "@/services/tauri";
import { useAppStore } from "@/store/appStore";

import { MemorySparkline } from "./MemorySparkline";

/** Files, bytes, speed and ETA of the download in progress (if any). */
export function TransferPanel() {
  const progress = useAppStore((s) => s.downloadProgress);
  const rate = useAppStore((s) => s.downloadRate);
  if (!progress || progress.files_total === 0 || progress.files_done >= progress.files_total) return null;

  const percent = progress.bytes_total > 0 ? Math.min(100, (progress.bytes_done / progress.bytes_total) * 100) : 0;
  const eta = formatEta(progress.bytes_total - progress.bytes_done, rate);

  return (
    <motion.div
      initial={{ opacity: 0, y: 8 }}
      animate={{ opacity: 1, y: 0 }}
      className="glass space-y-2.5 rounded-2xl p-4"
    >
      <div className="flex items-baseline justify-between gap-4">
        <p className="truncate text-sm font-semibold">{progress.label}</p>
        <p className="text-2xl font-black tabular-nums">{Math.round(percent)}%</p>
      </div>
      <div className="relative h-2.5 overflow-hidden rounded-full bg-muted">
        <motion.div
          className="bg-gradient-brand absolute inset-y-0 left-0 rounded-full"
          animate={{ width: `${percent}%` }}
          transition={{ ease: "easeOut" }}
        />
        <div className="shimmer-bg absolute inset-0 animate-shimmer" />
      </div>
      <div className="flex flex-wrap justify-between gap-x-4 text-xs text-muted-foreground tabular-nums">
        <span>
          {progress.files_done} / {progress.files_total} fichiers
        </span>
        <span>
          {formatBytes(progress.bytes_done)} / {formatBytes(progress.bytes_total)}
        </span>
        <span>{rate > 0 ? formatSpeed(rate) : "—"}</span>
        <span>{eta ? `≈ ${eta} restantes` : "Calcul…"}</span>
      </div>
    </motion.div>
  );
}

const HISTORY = 45;

function StatBox({ icon: Icon, label, value, sub }: { icon: typeof Cpu; label: string; value: string; sub?: string }) {
  return (
    <div className="flex items-center gap-3">
      <div className="flex size-9 items-center justify-center rounded-lg bg-primary/12 text-primary">
        <Icon className="size-4" aria-hidden="true" />
      </div>
      <div>
        <p className="text-lg leading-tight font-bold tabular-nums">{value}</p>
        <p className="text-xs text-muted-foreground">
          {label}
          {sub && <span className="text-muted-foreground/70"> · {sub}</span>}
        </p>
      </div>
    </div>
  );
}

/** Session clock, live RAM (with history) and CPU of the running game. */
export function LiveStatsPanel({ instanceId, allocatedMb }: { instanceId: string; allocatedMb: number }) {
  const startedAt = useAppStore((s) => s.runtime[instanceId]?.startedAt ?? null);
  const now = useNow(1000);
  const [history, setHistory] = useState<number[]>([]);
  const { data: stats } = useQuery({
    queryKey: ["process-stats", instanceId],
    queryFn: async () => {
      const next = await launchApi.stats(instanceId);
      if (next) setHistory((h) => [...h, next.memory_mb].slice(-HISTORY));
      return next;
    },
    refetchInterval: 2000,
    gcTime: 0,
  });

  return (
    <motion.div
      initial={{ opacity: 0, y: 8 }}
      animate={{ opacity: 1, y: 0 }}
      className="glass grid grid-cols-[auto_auto_auto_1fr] items-center gap-6 rounded-2xl p-4"
    >
      <StatBox icon={Timer} label="Session" value={startedAt ? formatClock((now - startedAt) / 1000) : "—"} />
      <StatBox
        icon={MemoryStick}
        label="RAM utilisée"
        value={stats ? formatGb(stats.memory_mb) : "—"}
        sub={`sur ${formatGb(allocatedMb)} allouées`}
      />
      <StatBox icon={Cpu} label="Processeur" value={stats ? `${Math.round(stats.cpu_percent)} %` : "—"} />
      <MemorySparkline samples={history} allocatedMb={allocatedMb} />
    </motion.div>
  );
}

interface CrashCardProps {
  analysis: CrashAnalysis;
  logs: RawLogLine[];
  onDismiss: () => void;
  onOpenFolder: () => void;
  onRelaunch: () => void;
}

export function CrashCard({ analysis, logs, onDismiss, onOpenFolder, onRelaunch }: CrashCardProps) {
  async function copyReport() {
    const lines = logs.slice(-300).map((l) => l.line);
    const report = [
      analysis.summary,
      analysis.suggestion ?? "",
      `Motif : ${analysis.matched_pattern}`,
      "",
      ...lines,
    ].join("\n");
    try {
      await navigator.clipboard.writeText(report);
      notify.success({ title: "Rapport de crash copié", history: false });
    } catch {
      notify.error({ title: "Impossible de copier dans le presse-papiers", history: false });
    }
  }

  return (
    <motion.div
      initial={{ opacity: 0, scale: 0.98 }}
      animate={{ opacity: 1, scale: 1 }}
      className={cn("relative overflow-hidden rounded-2xl border border-destructive/40 bg-destructive/10 p-4")}
      role="alert"
    >
      <div className="flex items-start gap-3">
        <div className="flex size-10 shrink-0 items-center justify-center rounded-xl bg-destructive/20">
          <AlertTriangle className="size-5 text-destructive" aria-hidden="true" />
        </div>
        <div className="min-w-0 flex-1 space-y-1">
          <p className="font-semibold text-destructive">{analysis.summary}</p>
          {analysis.suggestion && <p className="text-sm text-foreground/80">{analysis.suggestion}</p>}
          <div className="flex flex-wrap gap-2 pt-2">
            <Button size="sm" onClick={onRelaunch} className="gap-1.5">
              <RotateCcw aria-hidden="true" />
              Relancer
            </Button>
            <Button size="sm" variant="outline" onClick={copyReport} className="gap-1.5">
              <Copy aria-hidden="true" />
              Copier le rapport
            </Button>
            <Button size="sm" variant="outline" onClick={onOpenFolder} className="gap-1.5">
              <FolderOpen aria-hidden="true" />
              Ouvrir le dossier
            </Button>
          </div>
        </div>
        <Button variant="ghost" size="icon-sm" title="Fermer" onClick={onDismiss}>
          <X aria-hidden="true" />
        </Button>
      </div>
    </motion.div>
  );
}
