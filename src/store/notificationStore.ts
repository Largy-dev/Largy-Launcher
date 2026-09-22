import { create } from "zustand";

export type NotificationKind = "success" | "error" | "info" | "warning";

export interface NotificationAction {
  label: string;
  onClick: () => void;
}

export interface AppNotification {
  id: string;
  kind: NotificationKind;
  title: string;
  message?: string;
  createdAt: number;
  read: boolean;
  action?: NotificationAction;
}

/** How many past notifications the notification center keeps. */
export const MAX_NOTIFICATIONS = 50;

interface NotificationStore {
  items: AppNotification[];
  push: (notification: Omit<AppNotification, "id" | "createdAt" | "read">) => AppNotification;
  markAllRead: () => void;
  clear: () => void;
}

let counter = 0;

export const useNotifications = create<NotificationStore>((set) => ({
  items: [],
  push: (notification) => {
    const item: AppNotification = {
      ...notification,
      id: `${Date.now()}-${counter++}`,
      createdAt: Date.now(),
      read: false,
    };
    set((s) => ({ items: [item, ...s.items].slice(0, MAX_NOTIFICATIONS) }));
    return item;
  },
  markAllRead: () => set((s) => ({ items: s.items.map((n) => (n.read ? n : { ...n, read: true })) })),
  clear: () => set({ items: [] }),
}));

export function unreadCount(items: AppNotification[]): number {
  return items.reduce((n, item) => n + (item.read ? 0 : 1), 0);
}
