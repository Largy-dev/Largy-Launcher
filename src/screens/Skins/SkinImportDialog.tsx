import { useEffect, useState } from "react";
import { Loader2 } from "lucide-react";

import { ChoiceGroup } from "@/components/settings/SettingsKit";
import { SkinViewer3D } from "@/components/skins/SkinViewer3D";
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
import { Label } from "@/components/ui/label";
import { detectVariant } from "@/lib/skinPreview";
import type { SkinVariant } from "@/services/skins";

export interface PickedSkin {
  path: string;
  texture: string;
  name: string;
}

const VARIANTS: { value: SkinVariant; label: string }[] = [
  { value: "classic", label: "Classique" },
  { value: "slim", label: "Fin" },
];

interface SkinImportDialogProps {
  picked: PickedSkin | null;
  onOpenChange: (open: boolean) => void;
  /** `wear`: upload now (it's also kept in the library); otherwise only save it. */
  onConfirm: (skin: { path: string; name: string; variant: SkinVariant }, wear: boolean) => void;
  pending: boolean;
  canWear: boolean;
}

/** Name the imported skin, check its arm width on a 3D preview, then wear or save it. */
export function SkinImportDialog({ picked, onOpenChange, onConfirm, pending, canWear }: SkinImportDialogProps) {
  const [name, setName] = useState("");
  const [variant, setVariant] = useState<SkinVariant>("classic");

  useEffect(() => {
    if (!picked) return;
    let cancelled = false;
    // Seeds the form from the newly picked file; the arm width is detected async.
    // eslint-disable-next-line react-hooks/set-state-in-effect
    setName(picked.name);
    detectVariant(picked.texture)
      .then((v) => !cancelled && setVariant(v))
      .catch(() => {});
    return () => {
      cancelled = true;
    };
  }, [picked]);

  const submit = (wear: boolean) => picked && onConfirm({ path: picked.path, name, variant }, wear);

  return (
    <Dialog open={picked !== null} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-lg">
        <DialogHeader>
          <DialogTitle>Nouveau skin</DialogTitle>
          <DialogDescription>Vérifie l'aperçu : la largeur des bras doit correspondre à ton skin.</DialogDescription>
        </DialogHeader>
        <div className="flex gap-5">
          <div className="glass shrink-0 rounded-xl">
            {picked && <SkinViewer3D skin={picked.texture} cape={null} variant={variant} width={170} height={250} />}
          </div>
          <div className="flex-1 space-y-4">
            <div className="space-y-1.5">
              <Label htmlFor="skin-name">Nom</Label>
              <Input id="skin-name" maxLength={40} value={name} onChange={(e) => setName(e.target.value)} />
            </div>
            <div className="space-y-1.5">
              <Label>Bras</Label>
              <ChoiceGroup value={variant} options={VARIANTS} onChange={setVariant} />
              <p className="text-xs text-muted-foreground">Classique : 4 pixels (Steve). Fin : 3 pixels (Alex).</p>
            </div>
          </div>
        </div>
        <DialogFooter>
          <Button variant="outline" disabled={pending} onClick={() => submit(false)}>
            Ajouter à ma bibliothèque
          </Button>
          <Button
            className="bg-gradient-brand gap-1.5"
            disabled={pending || !canWear}
            title={canWear ? undefined : "Connecte un compte Microsoft pour porter un skin"}
            onClick={() => submit(true)}
          >
            {pending && <Loader2 className="animate-spin" aria-hidden="true" />}
            Porter maintenant
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
