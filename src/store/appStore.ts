import { create } from "zustand";

import type { AccountSession, DownloadProgress } from "@/services/tauri";

interface InstanceRuntime {
  running: boolean;
  logs: string[];
}

const emptyRuntime: InstanceRuntime = { running: false, logs: [] };

interface AppStore {
  activeInstanceId: string | null;
  setActiveInstanceId: (id: string | null) => void;

  account: AccountSession | null;
  setAccount: (account: AccountSession | null) => void;

  downloadProgress: DownloadProgress | null;
  setDownloadProgress: (progress: DownloadProgress | null) => void;

  runtime: Record<string, InstanceRuntime>;
  setRunning: (instanceId: string, running: boolean) => void;
  appendLog: (instanceId: string, line: string) => void;
  clearLogs: (instanceId: string) => void;
  runtimeFor: (instanceId: string) => InstanceRuntime;
}

export const useAppStore = create<AppStore>((set, get) => ({
  activeInstanceId: null,
  setActiveInstanceId: (id) => set({ activeInstanceId: id }),

  account: null,
  setAccount: (account) => set({ account }),

  downloadProgress: null,
  setDownloadProgress: (progress) => set({ downloadProgress: progress }),

  runtime: {},
  setRunning: (instanceId, running) =>
    set((s) => ({
      runtime: { ...s.runtime, [instanceId]: { ...(s.runtime[instanceId] ?? emptyRuntime), running } },
    })),
  appendLog: (instanceId, line) =>
    set((s) => {
      const current = s.runtime[instanceId] ?? emptyRuntime;
      const logs = [...current.logs, line].slice(-1000);
      return { runtime: { ...s.runtime, [instanceId]: { ...current, logs } } };
    }),
  clearLogs: (instanceId) =>
    set((s) => ({
      runtime: { ...s.runtime, [instanceId]: { ...(s.runtime[instanceId] ?? emptyRuntime), logs: [] } },
    })),
  runtimeFor: (instanceId) => get().runtime[instanceId] ?? emptyRuntime,
}));
