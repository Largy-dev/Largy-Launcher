import { cn } from "@/lib/utils";

export function Skeleton({ className }: { className?: string }) {
  return <div className={cn("shimmer-bg animate-shimmer rounded-lg bg-muted/70", className)} aria-hidden="true" />;
}
