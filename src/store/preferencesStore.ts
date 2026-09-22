import { create } from "zustand";
import { createJSONStorage, persist, type StateStorage } from "zustand/middleware";

import { DEFAULT_ACCENT, loadLegacyAccent, type Accent, type AnimationLevel, type ThemeMode } from "@/lib/theme";

export type BackgroundStyle = "image" | "gradient" | "solid";
export type CardDensity = "grid" | "compact" | "list";
export type InstanceSort = "recent" | "name" | "playtime";

export interface NotificationPreferences {
  /** Native Windows notifications when the launcher window isn't focused. */
  native: boolean;
  gameExit: boolean;
  crash: boolean;
  installDone: boolean;
  updates: boolean;
}

export interface LogPreferences {
  autoScroll: boolean;
  wrap: boolean;
}

/**
 * Purely visual, per-device preferences. Anything that changes how the game
 * launches lives in the backend's GlobalSettings instead.
 */
export interface Preferences {
  themeMode: ThemeMode;
  accent: Accent;
  customAccent: string;
  background: BackgroundStyle;
  blurIntensity: number;
  animations: AnimationLevel;
  cardDensity: CardDensity;
  instanceSort: InstanceSort;
  uiScale: number;
  notifications: NotificationPreferences;
  logs: LogPreferences;
}

export const DEFAULT_PREFERENCES: Preferences = {
  themeMode: "dark",
  accent: DEFAULT_ACCENT,
  customAccent: "#22c55e",
  background: "image",
  blurIntensity: 60,
  animations: "full",
  cardDensity: "grid",
  instanceSort: "recent",
  uiScale: 100,
  notifications: { native: true, gameExit: true, crash: true, installDone: true, updates: true },
  logs: { autoScroll: true, wrap: true },
};

interface PreferencesStore extends Preferences {
  set: (patch: Partial<Preferences>) => void;
  setNotifications: (patch: Partial<NotificationPreferences>) => void;
  setLogs: (patch: Partial<LogPreferences>) => void;
  reset: () => void;
  /** Back to the default look, keeping notification and log preferences. */
  resetAppearance: () => void;
}

/** localStorage that never throws — a blocked/private storage just doesn't persist. */
const safeStorage: StateStorage = {
  getItem: (name) => {
    try {
      return localStorage.getItem(name);
    } catch {
      return null;
    }
  },
  setItem: (name, value) => {
    try {
      localStorage.setItem(name, value);
    } catch {
      // Best-effort only.
    }
  },
  removeItem: (name) => {
    try {
      localStorage.removeItem(name);
    } catch {
      // Best-effort only.
    }
  },
};

export const PREFERENCES_STORAGE_KEY = "largy-preferences";

export const usePreferences = create<PreferencesStore>()(
  persist(
    (set) => ({
      ...DEFAULT_PREFERENCES,
      accent: loadLegacyAccent() ?? DEFAULT_PREFERENCES.accent,
      set: (patch) => set(patch),
      setNotifications: (patch) => set((s) => ({ notifications: { ...s.notifications, ...patch } })),
      setLogs: (patch) => set((s) => ({ logs: { ...s.logs, ...patch } })),
      reset: () => set(DEFAULT_PREFERENCES),
      resetAppearance: () => set((s) => ({ ...DEFAULT_PREFERENCES, notifications: s.notifications, logs: s.logs })),
    }),
    {
      name: PREFERENCES_STORAGE_KEY,
      version: 1,
      storage: createJSONStorage(() => safeStorage),
      // Nested objects are merged key by key so a preference added in a later
      // version still gets its default for users with an older saved blob.
      merge: (persisted, current) => {
        const saved = (persisted ?? {}) as Partial<Preferences>;
        return {
          ...current,
          ...saved,
          notifications: { ...current.notifications, ...saved.notifications },
          logs: { ...current.logs, ...saved.logs },
        };
      },
    },
  ),
);
