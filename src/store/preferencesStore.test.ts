import { beforeEach, describe, expect, it } from "vitest";

import { DEFAULT_PREFERENCES, PREFERENCES_STORAGE_KEY, usePreferences } from "./preferencesStore";

describe("usePreferences", () => {
  beforeEach(() => {
    localStorage.clear();
    usePreferences.getState().reset();
  });

  it("persists changes to localStorage", () => {
    usePreferences.getState().set({ accent: "cyan", cardDensity: "list" });
    const saved = JSON.parse(localStorage.getItem(PREFERENCES_STORAGE_KEY) ?? "{}");
    expect(saved.state.accent).toBe("cyan");
    expect(saved.state.cardDensity).toBe("list");
  });

  it("merges nested notification preferences instead of replacing them", () => {
    usePreferences.getState().setNotifications({ crash: false });
    const { notifications } = usePreferences.getState();
    expect(notifications.crash).toBe(false);
    expect(notifications.native).toBe(DEFAULT_PREFERENCES.notifications.native);
  });

  it("fills preferences missing from an older saved blob with their defaults", async () => {
    localStorage.setItem(
      PREFERENCES_STORAGE_KEY,
      JSON.stringify({ state: { accent: "red", notifications: { native: false } }, version: 1 }),
    );
    await usePreferences.persist.rehydrate();
    const state = usePreferences.getState();
    expect(state.accent).toBe("red");
    expect(state.notifications.native).toBe(false);
    expect(state.notifications.crash).toBe(true);
    expect(state.logs).toEqual(DEFAULT_PREFERENCES.logs);
  });
});
