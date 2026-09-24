import { create } from "zustand";

import type { RawLogLine } from "@/lib/logParse";
import type { AccountSession, CrashAnalysis, DownloadProgress, InstallWarning, LaunchPhase } from "@/services/tauri";

export interface PendingInstallWarnings {
  instanceId: string;
  instanceName: string;
  warnings: InstallWarning[];
}

export interface InstanceRuntime {
  running: boolean;
  logs: RawLogLine[];
  /** Current launch step, `running` once the game process is up, null when idle. */
  phase: LaunchPhase | null;
  /** Epoch ms when the game process started, for the session timer. */
  startedAt: number | null;
  /** The user asked to stop it — its non-zero exit code isn't a crash. */
  stopping: boolean;
}

const emptyRuntime: InstanceRuntime = { running: false, logs: [], phase: null, startedAt: null, stopping: false };

/** Kept per launch — enough for a crash trace without growing forever over a long session. */
const MAX_LOG_LINES = 2000;

interface AppStore {
  activeInstanceId: string | null;
  setActiveInstanceId: (id: string | null) => void;

  account: AccountSession | null;
  /** The startup silent login has settled — `account` is meaningful from then on. */
  authReady: boolean;
  setAccount: (account: AccountSession | null) => void;

  downloadProgress: DownloadProgress | null;
  /** Smoothed transfer speed of the current download, in bytes/s. */
  downloadRate: number;
  downloadSample: { at: number; bytes: number };
  setDownloadProgress: (progress: DownloadProgress | null) => void;

  runtime: Record<string, InstanceRuntime>;
  setRunning: (instanceId: string, running: boolean) => void;
  markStopping: (instanceId: string) => void;
  setPhase: (instanceId: string, phase: LaunchPhase) => void;
  appendLogs: (instanceId: string, lines: RawLogLine[]) => void;
  clearLogs: (instanceId: string) => void;

  crashAnalysis: Record<string, CrashAnalysis | null>;
  setCrashAnalysis: (instanceId: string, analysis: CrashAnalysis | null) => void;
  clearCrashAnalysis: (instanceId: string) => void;

  installWarnings: PendingInstallWarnings | null;
  setInstallWarnings: (warnings: PendingInstallWarnings) => void;
  clearInstallWarnings: () => void;
}

export const useAppStore = create<AppStore>((set) => {
  const patchRuntime = (instanceId: string, patch: (current: InstanceRuntime) => Partial<InstanceRuntime>) =>
    set((s) => {
      const current = s.runtime[instanceId] ?? emptyRuntime;
      return { runtime: { ...s.runtime, [instanceId]: { ...current, ...patch(current) } } };
    });

  return {
    activeInstanceId: null,
    setActiveInstanceId: (id) => set({ activeInstanceId: id }),

    account: null,
    authReady: false,
    setAccount: (account) => set({ account, authReady: true }),

    downloadProgress: null,
    downloadRate: 0,
    downloadSample: { at: 0, bytes: 0 },
    setDownloadProgress: (progress) =>
      set((s) => {
        const now = Date.now();
        const previous = s.downloadProgress;
        const sameTask =
          !!progress && !!previous && previous.task_id === progress.task_id && previous.label === progress.label;
        if (!progress || !sameTask) {
          return {
            downloadProgress: progress,
            downloadRate: 0,
            downloadSample: { at: now, bytes: progress?.bytes_done ?? 0 },
          };
        }
        const elapsed = (now - s.downloadSample.at) / 1000;
        if (elapsed < 0.5) return { downloadProgress: progress };
        const instant = Math.max(0, progress.bytes_done - s.downloadSample.bytes) / elapsed;
        return {
          downloadProgress: progress,
          downloadRate: s.downloadRate === 0 ? instant : s.downloadRate * 0.6 + instant * 0.4,
          downloadSample: { at: now, bytes: progress.bytes_done },
        };
      }),

    runtime: {},
    setRunning: (instanceId, running) =>
      patchRuntime(instanceId, () =>
        running ? { running, stopping: false } : { running, phase: null, startedAt: null, stopping: false },
      ),
    markStopping: (instanceId) => patchRuntime(instanceId, () => ({ stopping: true })),
    setPhase: (instanceId, phase) =>
      patchRuntime(instanceId, (current) => ({
        phase,
        startedAt: phase === "running" ? (current.startedAt ?? Date.now()) : current.startedAt,
      })),
    appendLogs: (instanceId, lines) =>
      patchRuntime(instanceId, (current) => {
        const next = current.logs.concat(lines);
        return { logs: next.length > MAX_LOG_LINES ? next.slice(-MAX_LOG_LINES) : next };
      }),
    clearLogs: (instanceId) => patchRuntime(instanceId, () => ({ logs: [] })),

    crashAnalysis: {},
    setCrashAnalysis: (instanceId, analysis) =>
      set((s) => ({ crashAnalysis: { ...s.crashAnalysis, [instanceId]: analysis } })),
    clearCrashAnalysis: (instanceId) => set((s) => ({ crashAnalysis: { ...s.crashAnalysis, [instanceId]: null } })),

    installWarnings: null,
    setInstallWarnings: (warnings) => set({ installWarnings: warnings }),
    clearInstallWarnings: () => set({ installWarnings: null }),
  };
});

export function runtimeOf(runtime: Record<string, InstanceRuntime>, instanceId: string): InstanceRuntime {
  return runtime[instanceId] ?? emptyRuntime;
}
