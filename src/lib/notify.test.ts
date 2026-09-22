import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("sonner", () => ({
  toast: { success: vi.fn(), error: vi.fn(), info: vi.fn(), warning: vi.fn() },
}));
vi.mock("@tauri-apps/api/window", () => ({ getCurrentWindow: () => ({ isFocused: async () => true }) }));
vi.mock("@tauri-apps/plugin-notification", () => ({
  isPermissionGranted: vi.fn(),
  requestPermission: vi.fn(),
  sendNotification: vi.fn(),
}));

import { toast } from "sonner";

import { DEFAULT_PREFERENCES } from "@/store/preferencesStore";
import { useNotifications } from "@/store/notificationStore";

import { notify, shouldSendNative } from "./notify";

describe("shouldSendNative", () => {
  const prefs = DEFAULT_PREFERENCES.notifications;

  it("only fires for an enabled category while the window is in the background", () => {
    expect(shouldSendNative(prefs, "crash", false)).toBe(true);
    expect(shouldSendNative(prefs, "crash", true)).toBe(false);
    expect(shouldSendNative(prefs, undefined, false)).toBe(false);
    expect(shouldSendNative({ ...prefs, crash: false }, "crash", false)).toBe(false);
    expect(shouldSendNative({ ...prefs, native: false }, "crash", false)).toBe(false);
  });
});

describe("notify", () => {
  beforeEach(() => useNotifications.getState().clear());

  it("shows a toast and records the notification as unread", () => {
    notify.error({ title: "Crash", message: "OutOfMemory" });
    expect(toast.error).toHaveBeenCalledWith("Crash", expect.objectContaining({ description: "OutOfMemory" }));
    const [item] = useNotifications.getState().items;
    expect(item).toMatchObject({ kind: "error", title: "Crash", read: false });
  });

  it("can skip the history", () => {
    notify.success({ title: "Copié", history: false });
    expect(useNotifications.getState().items).toHaveLength(0);
  });
});
