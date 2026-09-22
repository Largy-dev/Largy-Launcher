import { Anvil, Blocks, Hammer, Layers, Scissors, type LucideIcon } from "lucide-react";

import { cn } from "@/lib/utils";
import type { LoaderKind } from "@/services/tauri";

export const LOADER_META: Record<LoaderKind, { label: string; color: string; icon: LucideIcon }> = {
  vanilla: { label: "Vanilla", color: "var(--loader-vanilla)", icon: Blocks },
  forge: { label: "Forge", color: "var(--loader-forge)", icon: Anvil },
  neoforge: { label: "NeoForge", color: "var(--loader-neoforge)", icon: Hammer },
  fabric: { label: "Fabric", color: "var(--loader-fabric)", icon: Scissors },
  quilt: { label: "Quilt", color: "var(--loader-quilt)", icon: Layers },
};

export const LOADER_ORDER: LoaderKind[] = ["vanilla", "fabric", "quilt", "forge", "neoforge"];

interface LoaderBadgeProps {
  loader: LoaderKind;
  version?: string | null;
  className?: string;
}

/** Loader name tinted with its own color — the same everywhere an instance appears. */
export function LoaderBadge({ loader, version, className }: LoaderBadgeProps) {
  const meta = LOADER_META[loader];
  const Icon = meta.icon;
  return (
    <span
      className={cn(
        "inline-flex h-5 shrink-0 items-center gap-1 rounded-full px-2 text-[0.7rem] font-semibold whitespace-nowrap",
        className,
      )}
      style={{
        color: `color-mix(in oklab, ${meta.color} 80%, var(--foreground))`,
        backgroundColor: `color-mix(in oklab, ${meta.color} 16%, transparent)`,
        boxShadow: `inset 0 0 0 1px color-mix(in oklab, ${meta.color} 30%, transparent)`,
      }}
    >
      <Icon className="size-3" aria-hidden="true" />
      {meta.label}
      {version && <span className="font-normal opacity-75">{version}</span>}
    </span>
  );
}
