import type { LucideIcon } from "lucide-react";
import type { ReactNode } from "react";

interface EmptyStateProps {
  icon: LucideIcon;
  title: string;
  description: string;
  action?: ReactNode;
}

export function EmptyState({ icon: Icon, title, description, action }: EmptyStateProps) {
  return (
    <div className="glass flex flex-1 flex-col items-center justify-center gap-4 rounded-2xl px-6 py-16 text-center">
      <div className="relative">
        <div className="bg-gradient-brand absolute inset-0 rounded-2xl opacity-40 blur-xl" aria-hidden="true" />
        <div className="bg-gradient-brand relative flex size-14 animate-float items-center justify-center rounded-2xl shadow-glow">
          <Icon className="size-6 text-primary-foreground" aria-hidden="true" />
        </div>
      </div>
      <div className="space-y-1">
        <h3 className="text-base font-semibold">{title}</h3>
        <p className="max-w-sm text-sm text-muted-foreground">{description}</p>
      </div>
      {action}
    </div>
  );
}
