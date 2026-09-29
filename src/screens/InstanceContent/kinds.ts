import { Globe, Image as ImageIcon, Puzzle, Sparkles, type LucideIcon } from "lucide-react";

import type { ContentKind } from "@/services/tauri";

export type ContentTab = ContentKind | "worlds";

interface KindMeta {
  label: string;
  icon: LucideIcon;
  folder: string;
  extension: string;
  searchPlaceholder: string;
  emptyTitle: string;
  emptyDescription: string;
}

export const KIND_META: Record<ContentKind, KindMeta> = {
  mod: {
    label: "Mods",
    icon: Puzzle,
    folder: "mods",
    extension: "jar",
    searchPlaceholder: "Rechercher un mod…",
    emptyTitle: "Aucun mod",
    emptyDescription: "Parcours Modrinth et CurseForge, ou glisse des fichiers .jar sur la fenêtre.",
  },
  resource_pack: {
    label: "Resource packs",
    icon: ImageIcon,
    folder: "resourcepacks",
    extension: "zip",
    searchPlaceholder: "Rechercher un pack…",
    emptyTitle: "Aucun resource pack",
    emptyDescription: "Change les textures et les sons du jeu. Parcours le catalogue ou glisse un .zip ici.",
  },
  shader: {
    label: "Shaders",
    icon: Sparkles,
    folder: "shaderpacks",
    extension: "zip",
    searchPlaceholder: "Rechercher un shader…",
    emptyTitle: "Aucun shader",
    emptyDescription:
      "Lumières, ombres et reflets réalistes (Iris ou OptiFine requis). Glisse un .zip ou parcours le catalogue.",
  },
};

export const WORLDS_META = { label: "Mondes", icon: Globe };
