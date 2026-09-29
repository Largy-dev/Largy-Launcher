import { useState } from "react";
import { AnimatePresence, motion } from "motion/react";
import { ChevronRight, Lightbulb, X } from "lucide-react";

import { Button } from "@/components/ui/button";
import { usePreferences } from "@/store/preferencesStore";

const TIPS = [
  "Glisse un .mrpack, un zip CurseForge ou une instance Prism sur la fenêtre pour l'importer.",
  "Ctrl+K ouvre la recherche rapide : saute vers n'importe quelle instance ou page.",
  "Un ami a mis son pack en ligne ? Modpacks › Rejoindre par lien : il se mettra à jour tout seul.",
  "Contenu › Mods › Optimiser installe les mods de performance adaptés à ton instance en un clic.",
  "Un crash ? « Partager » envoie le log sur mclo.gs et copie le lien, prêt à coller sur Discord.",
  "Avant chaque mise à jour de mods, un point de restauration est créé : Réglages de l'instance › Général.",
  "Paramètres › Jeu & Java : active les options partagées pour retrouver tes touches dans toutes tes instances.",
] as const;

/** One tip at a time on the home screen, until the player hides them. */
export function TipsCard() {
  const dismissed = usePreferences((s) => s.tipsDismissed);
  const setPrefs = usePreferences((s) => s.set);
  const [index, setIndex] = useState(() => Math.floor(Math.random() * TIPS.length));
  if (dismissed) return null;

  return (
    <div className="glass mb-5 flex items-center gap-3 rounded-xl py-2 pr-2 pl-3.5 text-sm" role="note">
      <Lightbulb className="size-4 shrink-0 text-amber-500" aria-hidden="true" />
      <div className="relative min-h-5 flex-1 overflow-hidden">
        <AnimatePresence mode="wait" initial={false}>
          <motion.p
            key={index}
            initial={{ opacity: 0, y: 6 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: -6 }}
            transition={{ duration: 0.18 }}
            className="text-muted-foreground"
          >
            <span className="font-semibold text-foreground">Astuce · </span>
            {TIPS[index]}
          </motion.p>
        </AnimatePresence>
      </div>
      <Button
        variant="ghost"
        size="icon-sm"
        aria-label="Astuce suivante"
        title="Astuce suivante"
        onClick={() => setIndex((i) => (i + 1) % TIPS.length)}
      >
        <ChevronRight aria-hidden="true" />
      </Button>
      <Button
        variant="ghost"
        size="icon-sm"
        aria-label="Ne plus afficher les astuces"
        title="Ne plus afficher les astuces"
        onClick={() => setPrefs({ tipsDismissed: true })}
      >
        <X aria-hidden="true" />
      </Button>
    </div>
  );
}
