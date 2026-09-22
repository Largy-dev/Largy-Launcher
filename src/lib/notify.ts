import { getCurrentWindow } from "@tauri-apps/api/window";
import { isPermissionGranted, requestPermission, sendNotification } from "@tauri-apps/plugin-notification";
import { toast } from "sonner";

import { useNotifications, type NotificationAction, type NotificationKind } from "@/store/notificationStore";
import { usePreferences, type NotificationPreferences } from "@/store/preferencesStore";

export type NativeCategory = Exclude<keyof NotificationPreferences, "native">;

export interface NotifyOptions {
  title: string;
  message?: string;
  action?: NotificationAction;
  /** Also raise a Windows notification for this category when the window isn't focused. */
  native?: NativeCategory;
  /** Keep the toast until dismissed. */
  sticky?: boolean;
  /** Record in the notification center (default true). */
  history?: boolean;
}

export function shouldSendNative(
  prefs: NotificationPreferences,
  category: NativeCategory | undefined,
  windowFocused: boolean,
): boolean {
  return !!category && prefs.native && prefs[category] && !windowFocused;
}

async function sendNative(title: string, body: string | undefined, category: NativeCategory | undefined) {
  try {
    const focused = await getCurrentWindow().isFocused();
    if (!shouldSendNative(usePreferences.getState().notifications, category, focused)) return;
    let granted = await isPermissionGranted();
    if (!granted) granted = (await requestPermission()) === "granted";
    if (granted) sendNotification({ title, body });
  } catch {
    // Native notifications are a bonus — never let them break the in-app flow.
  }
}

function show(kind: NotificationKind, options: NotifyOptions): string | number {
  if (options.history !== false) {
    useNotifications.getState().push({
      kind,
      title: options.title,
      message: options.message,
      action: options.action,
    });
  }
  if (options.native) void sendNative(options.title, options.message, options.native);
  return toast[kind](options.title, {
    description: options.message,
    action: options.action,
    duration: options.sticky ? Infinity : undefined,
  });
}

/** Single entry point for user-facing feedback: toast + notification center + optional native notification. */
export const notify = {
  success: (options: NotifyOptions) => show("success", options),
  error: (options: NotifyOptions) => show("error", options),
  info: (options: NotifyOptions) => show("info", options),
  warning: (options: NotifyOptions) => show("warning", options),
};
