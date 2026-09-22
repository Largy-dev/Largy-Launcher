import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { useAppStore } from "./appStore";

const progress = (bytes_done: number, label = "Assets") => ({
  task_id: "t",
  label,
  bytes_done,
  bytes_total: 10_000_000,
  files_done: 1,
  files_total: 10,
});

describe("download rate", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    useAppStore.getState().setDownloadProgress(null);
  });
  afterEach(() => vi.useRealTimers());

  it("measures bytes per second between samples and resets on a new task", () => {
    const { setDownloadProgress } = useAppStore.getState();
    setDownloadProgress(progress(0));
    vi.advanceTimersByTime(1000);
    setDownloadProgress(progress(2_000_000));
    expect(useAppStore.getState().downloadRate).toBe(2_000_000);

    setDownloadProgress(progress(0, "Bibliothèques"));
    expect(useAppStore.getState().downloadRate).toBe(0);
  });
});

describe("runtime", () => {
  it("records the session start once the game is running and clears it on exit", () => {
    const store = useAppStore.getState();
    store.setRunning("a", true);
    store.setPhase("a", "running");
    expect(useAppStore.getState().runtime.a.startedAt).not.toBeNull();
    store.setRunning("a", false);
    expect(useAppStore.getState().runtime.a).toMatchObject({ running: false, phase: null, startedAt: null });
  });
});
