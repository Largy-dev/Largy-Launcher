import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { useNavigate } from "react-router";
import { motion } from "motion/react";
import { ArrowRightLeft, Compass, FileDown, Link2, Sparkles, type LucideIcon } from "lucide-react";

import { useImportInstance } from "@/hooks/useImportInstance";
import { fadeUp, stagger } from "@/lib/motion";
import { CreateInstanceDialog } from "@/screens/InstanceList/CreateInstanceDialog";
import { ExternalImportDialog } from "@/screens/InstanceList/ExternalImportDialog";
import { JoinPackDialog } from "@/screens/ModpackBrowser/JoinPackDialog";
import { externalApi } from "@/services/external";

function Choice({
  icon: Icon,
  title,
  description,
  onClick,
  highlight,
}: {
  icon: LucideIcon;
  title: string;
  description: string;
  onClick: () => void;
  highlight?: boolean;
}) {
  return (
    <motion.button
      variants={fadeUp}
      whileHover={{ y: -3 }}
      onClick={onClick}
      className="panel flex items-start gap-4 p-5 text-left transition-colors hover:bg-[var(--surface-hover)]"
    >
      <span
        className={
          highlight
            ? "bg-gradient-brand shadow-glow flex size-11 shrink-0 items-center justify-center rounded-xl text-primary-foreground"
            : "flex size-11 shrink-0 items-center justify-center rounded-xl bg-foreground/8"
        }
      >
        <Icon className="size-5" aria-hidden="true" />
      </span>
      <span>
        <span className="block font-semibold">{title}</span>
        <span className="mt-0.5 block text-sm text-muted-foreground">{description}</span>
      </span>
    </motion.button>
  );
}

/** First run: no instance yet — every way to get one, the quickest first. */
export function Welcome() {
  const navigate = useNavigate();
  const [createOpen, setCreateOpen] = useState(false);
  const [externalOpen, setExternalOpen] = useState(false);
  const [joinOpen, setJoinOpen] = useState(false);
  const { pickAndImport } = useImportInstance();
  const { data: external } = useQuery({
    queryKey: ["external-instances"],
    queryFn: externalApi.detect,
    staleTime: 60_000,
  });
  const found = external?.filter((e) => !e.already_imported).length ?? 0;

  return (
    <motion.div variants={stagger} initial="hidden" animate="show" className="mx-auto w-full max-w-4xl pt-12">
      <motion.p variants={fadeUp} className="text-sm font-semibold text-primary">
        Bienvenue
      </motion.p>
      <motion.h1 variants={fadeUp} className="mt-1 text-5xl font-black tracking-tight">
        Prêt pour l'aventure ?
      </motion.h1>
      <motion.p variants={fadeUp} className="mt-3 max-w-xl text-foreground/75">
        Crée ta première instance, installe un modpack ou récupère celles de tes autres launchers. Tu peux aussi glisser
        un fichier .mrpack ou .zip n'importe où sur la fenêtre.
      </motion.p>

      <div className="mt-8 grid gap-3 sm:grid-cols-2">
        {found > 0 && (
          <Choice
            highlight
            icon={ArrowRightLeft}
            title={`${found} instance${found > 1 ? "s" : ""} trouvée${found > 1 ? "s" : ""} ailleurs`}
            description="Récupère tes mods, réglages et mondes en quelques clics — sans toucher à l'original."
            onClick={() => setExternalOpen(true)}
          />
        )}
        <Choice
          highlight={found === 0}
          icon={Sparkles}
          title="Nouvelle instance"
          description="Vanilla, Fabric, Forge, NeoForge ou Quilt, dans la version de ton choix."
          onClick={() => setCreateOpen(true)}
        />
        <Choice
          icon={Compass}
          title="Parcourir les modpacks"
          description="Des milliers de packs Modrinth, CurseForge et FTB, installés en un clic."
          onClick={() => navigate("/modpacks")}
        />
        <Choice
          icon={FileDown}
          title="Importer un fichier"
          description=".mrpack, zip CurseForge ou export Prism / MultiMC."
          onClick={pickAndImport}
        />
        <Choice
          icon={Link2}
          title="Rejoindre un pack par lien"
          description="Le .mrpack partagé par un ami ou un serveur, mis à jour tout seul."
          onClick={() => setJoinOpen(true)}
        />
        {found === 0 && (
          <Choice
            icon={ArrowRightLeft}
            title="Depuis un autre launcher"
            description="Officiel, Prism, CurseForge ou Modrinth App."
            onClick={() => setExternalOpen(true)}
          />
        )}
      </div>

      <CreateInstanceDialog open={createOpen} onOpenChange={setCreateOpen} />
      <ExternalImportDialog open={externalOpen} onOpenChange={setExternalOpen} />
      <JoinPackDialog open={joinOpen} onOpenChange={setJoinOpen} />
    </motion.div>
  );
}
