import type { ReactNode } from "react";

interface PageHeaderProps {
  title: ReactNode;
  description?: ReactNode;
  action?: ReactNode;
  /** Small line above the title (breadcrumb, category…). */
  eyebrow?: ReactNode;
}

export function PageHeader({ title, description, action, eyebrow }: PageHeaderProps) {
  return (
    <div className="mb-6 flex items-end justify-between gap-4">
      <div className="min-w-0">
        {eyebrow && <div className="mb-1 text-xs font-semibold tracking-wider text-primary uppercase">{eyebrow}</div>}
        <h2 className="truncate text-2xl font-bold tracking-tight">{title}</h2>
        {description && <div className="mt-1 text-sm text-muted-foreground">{description}</div>}
      </div>
      {action && <div className="flex shrink-0 items-center gap-2">{action}</div>}
    </div>
  );
}
