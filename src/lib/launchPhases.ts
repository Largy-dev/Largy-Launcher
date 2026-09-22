import { Coffee, Download, Gamepad2, KeyRound, Library, Puzzle, Rocket, type LucideIcon } from "lucide-react";

import type { LaunchPhase, LoaderKind } from "@/services/tauri";

export const PHASE_META: Record<LaunchPhase, { label: string; hint: string; icon: LucideIcon }> = {
  auth: { label: "Compte", hint: "Vérification de ta session", icon: KeyRound },
  version: { label: "Minecraft", hint: "Fichiers du jeu, bibliothèques et assets", icon: Download },
  loader: { label: "Mod loader", hint: "Installation et vérification du loader", icon: Puzzle },
  natives: { label: "Natifs", hint: "Extraction des bibliothèques natives", icon: Library },
  java: { label: "Java", hint: "Le bon runtime Java pour cette version", icon: Coffee },
  starting: { label: "Démarrage", hint: "Lancement de la JVM", icon: Rocket },
  running: { label: "En jeu", hint: "Bonne partie !", icon: Gamepad2 },
};

/** The steps a launch of this instance goes through, in order. */
export function phasesFor(loader: LoaderKind): LaunchPhase[] {
  const phases: LaunchPhase[] = ["auth", "version"];
  if (loader !== "vanilla") phases.push("loader");
  phases.push("natives", "java", "starting", "running");
  return phases;
}
