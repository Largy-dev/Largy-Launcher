import { AnimatePresence, motion } from "motion/react";
import { Bell, CheckCheck, CircleCheck, Info, OctagonX, Trash2, TriangleAlert, type LucideIcon } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { formatRelative } from "@/lib/format";
import { cn } from "@/lib/utils";
import { unreadCount, useNotifications, type NotificationKind } from "@/store/notificationStore";

const KIND_STYLE: Record<NotificationKind, { icon: LucideIcon; className: string }> = {
  success: { icon: CircleCheck, className: "text-success bg-success/12" },
  error: { icon: OctagonX, className: "text-destructive bg-destructive/12" },
  warning: { icon: TriangleAlert, className: "text-warning bg-warning/12" },
  info: { icon: Info, className: "text-info bg-info/12" },
};

export function NotificationCenter() {
  const items = useNotifications((s) => s.items);
  const markAllRead = useNotifications((s) => s.markAllRead);
  const clear = useNotifications((s) => s.clear);
  const unread = unreadCount(items);

  return (
    <Popover onOpenChange={(open) => !open && markAllRead()}>
      <PopoverTrigger asChild>
        <button
          title="Notifications"
          aria-label={`Notifications${unread ? ` (${unread} non lues)` : ""}`}
          className="relative flex size-8 items-center justify-center rounded-lg text-sidebar-foreground/70 transition-colors hover:bg-sidebar-accent hover:text-sidebar-foreground"
        >
          <Bell className="size-4" aria-hidden="true" />
          <AnimatePresence>
            {unread > 0 && (
              <motion.span
                initial={{ scale: 0 }}
                animate={{ scale: 1 }}
                exit={{ scale: 0 }}
                className="absolute -top-0.5 -right-0.5 flex h-4 min-w-4 items-center justify-center rounded-full bg-primary px-1 text-[0.6rem] font-bold text-primary-foreground"
              >
                {unread > 9 ? "9+" : unread}
              </motion.span>
            )}
          </AnimatePresence>
        </button>
      </PopoverTrigger>
      <PopoverContent side="right" align="start" className="w-80 p-0">
        <div className="flex items-center justify-between border-b border-border px-3 py-2">
          <p className="text-sm font-semibold">Notifications</p>
          <div className="flex gap-1">
            <Button variant="ghost" size="icon-xs" title="Tout marquer comme lu" onClick={markAllRead}>
              <CheckCheck aria-hidden="true" />
            </Button>
            <Button variant="ghost" size="icon-xs" title="Tout effacer" onClick={clear} disabled={items.length === 0}>
              <Trash2 aria-hidden="true" />
            </Button>
          </div>
        </div>
        <div className="max-h-96 overflow-y-auto p-1.5">
          {items.length === 0 ? (
            <p className="px-3 py-8 text-center text-sm text-muted-foreground">Rien de neuf pour l'instant.</p>
          ) : (
            items.map((item) => {
              const style = KIND_STYLE[item.kind];
              const Icon = style.icon;
              return (
                <div key={item.id} className={cn("flex gap-2.5 rounded-lg px-2 py-2", !item.read && "bg-accent/60")}>
                  <div
                    className={cn(
                      "mt-0.5 flex size-7 shrink-0 items-center justify-center rounded-md",
                      style.className,
                    )}
                  >
                    <Icon className="size-3.5" aria-hidden="true" />
                  </div>
                  <div className="min-w-0 flex-1">
                    <p className="text-sm leading-snug font-medium">{item.title}</p>
                    {item.message && <p className="line-clamp-3 text-xs text-muted-foreground">{item.message}</p>}
                    <div className="mt-1 flex items-center gap-2">
                      <span className="text-[0.7rem] text-muted-foreground/80">
                        {formatRelative(item.createdAt / 1000)}
                      </span>
                      {item.action && (
                        <button
                          onClick={item.action.onClick}
                          className="text-[0.7rem] font-medium text-primary hover:underline"
                        >
                          {item.action.label}
                        </button>
                      )}
                    </div>
                  </div>
                </div>
              );
            })
          )}
        </div>
      </PopoverContent>
    </Popover>
  );
}
