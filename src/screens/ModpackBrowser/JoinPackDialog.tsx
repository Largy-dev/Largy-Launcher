import { useState } from "react";
import { useMutation } from "@tanstack/react-query";
import { Link2, Loader2 } from "lucide-react";

import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { errorMessage, providersApi, type ModpackSummary } from "@/services/tauri";

import { ModpackDetailDialog } from "./ModpackDetailDialog";

interface JoinPackDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

/**
 * A pack shared by link (an `.mrpack` put online by a friend or a server):
 * the link is checked, then installed like any modpack — and kept in sync
 * with the file before every launch.
 */
export function JoinPackDialog({ open, onOpenChange }: JoinPackDialogProps) {
  const [url, setUrl] = useState("");
  const [pack, setPack] = useState<ModpackSummary | null>(null);

  const check = useMutation({
    mutationFn: (link: string) => providersApi.getModpack("url", link.trim()),
    onSuccess: (details) => {
      onOpenChange(false);
      setPack(details.summary);
    },
  });

  return (
    <>
      <Dialog
        open={open}
        onOpenChange={(o) => {
          onOpenChange(o);
          if (!o) check.reset();
        }}
      >
        <DialogContent className="sm:max-w-lg">
          <DialogHeader>
            <DialogTitle>Rejoindre un pack par lien</DialogTitle>
            <DialogDescription>
              Colle le lien d'un fichier <span className="font-mono">.mrpack</span> partagé par un ami ou un serveur.
              Dès qu'il est modifié, ton instance se met à jour toute seule avant de jouer.
            </DialogDescription>
          </DialogHeader>
          <form
            className="space-y-2"
            onSubmit={(e) => {
              e.preventDefault();
              if (url.trim()) check.mutate(url);
            }}
          >
            <div className="relative">
              <Link2
                className="pointer-events-none absolute top-1/2 left-3 size-4 -translate-y-1/2 text-muted-foreground"
                aria-hidden="true"
              />
              <Input
                autoFocus
                aria-label="Lien du pack"
                placeholder="https://…/mon-pack.mrpack"
                value={url}
                onChange={(e) => setUrl(e.target.value)}
                className="pl-9!"
              />
            </div>
            {check.isError && <p className="text-sm text-destructive">{errorMessage(check.error)}</p>}
            <p className="text-xs text-muted-foreground">
              Pour partager ta propre instance : Réglages de l'instance › Exporter en .mrpack, puis mets le fichier en
              ligne (GitHub, Dropbox…) et donne le lien. Les liens de partage GitHub et Dropbox sont convertis
              automatiquement.
            </p>
          </form>
          <DialogFooter>
            <Button variant="outline" onClick={() => onOpenChange(false)}>
              Annuler
            </Button>
            <Button disabled={!url.trim() || check.isPending} onClick={() => check.mutate(url)} className="gap-1.5">
              {check.isPending && <Loader2 className="animate-spin" aria-hidden="true" />}
              Continuer
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
      <ModpackDetailDialog provider="url" pack={pack} onOpenChange={(o) => !o && setPack(null)} />
    </>
  );
}
