import {
  Globe,
  Images,
  LayoutGrid,
  Package,
  Puzzle,
  Settings2,
  SquareTerminal,
  Users,
  type LucideIcon,
} from "lucide-react";

import type { ContentSummary } from "@/services/content";
import type { Instance } from "@/services/tauri";

export type InstanceTab = "overview" | "mods" | "packs" | "worlds" | "servers" | "screenshots" | "console" | "settings";

export interface TabMeta {
  id: InstanceTab;
  label: string;
  icon: LucideIcon;
}

const TABS: TabMeta[] = [
  { id: "overview", label: "Aperçu", icon: LayoutGrid },
  { id: "mods", label: "Mods", icon: Puzzle },
  { id: "packs", label: "Packs", icon: Package },
  { id: "worlds", label: "Mondes", icon: Globe },
  { id: "servers", label: "Multijoueur", icon: Users },
  { id: "screenshots", label: "Captures", icon: Images },
  { id: "console", label: "Console", icon: SquareTerminal },
  { id: "settings", label: "Réglages", icon: Settings2 },
];

/** The tabs this instance has — no Mods tab on vanilla. */
export function tabsFor(instance: Instance): TabMeta[] {
  return TABS.filter((t) => t.id !== "mods" || instance.loader !== "vanilla");
}

export function resolveTab(instance: Instance, requested: string | null): InstanceTab {
  return tabsFor(instance).find((t) => t.id === requested)?.id ?? "overview";
}

/** Little number next to a tab label, when there's something to count. */
export function tabCount(tab: InstanceTab, summary: ContentSummary | undefined): number | null {
  if (!summary) return null;
  switch (tab) {
    case "mods":
      return summary.mods;
    case "packs":
      return summary.resource_packs + summary.shaders;
    case "worlds":
      return summary.worlds;
    default:
      return null;
  }
}
