import { useState } from "react";
import { ExternalLink, Loader2 } from "lucide-react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { toast } from "sonner";

import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { auth, errorMessage, type DeviceCodeInfo } from "@/services/tauri";
import { useAppStore } from "@/store/appStore";

function MicrosoftMark() {
  return (
    <svg viewBox="0 0 21 21" className="size-4" aria-hidden="true">
      <rect x="1" y="1" width="9" height="9" fill="#f25022" />
      <rect x="11" y="1" width="9" height="9" fill="#7fba00" />
      <rect x="1" y="11" width="9" height="9" fill="#00a4ef" />
      <rect x="11" y="11" width="9" height="9" fill="#ffb900" />
    </svg>
  );
}

type LoginStatus = "idle" | "waiting" | "polling" | "error";

interface LoginDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

export function LoginDialog({ open, onOpenChange }: LoginDialogProps) {
  const setAccount = useAppStore((s) => s.setAccount);
  const [device, setDevice] = useState<DeviceCodeInfo | null>(null);
  const [status, setStatus] = useState<LoginStatus>("idle");
  const [error, setError] = useState<string | null>(null);

  function reset() {
    setDevice(null);
    setStatus("idle");
    setError(null);
  }

  async function startLogin() {
    setStatus("waiting");
    setError(null);
    try {
      const info = await auth.beginLogin();
      setDevice(info);
      setStatus("polling");
      const session = await auth.completeLogin(info);
      setAccount(session);
      toast.success(`Connecté en tant que ${session.profile.name}`);
      onOpenChange(false);
      reset();
    } catch (e) {
      setError(errorMessage(e));
      setStatus("error");
    }
  }

  return (
    <Dialog
      open={open}
      onOpenChange={(next) => {
        onOpenChange(next);
        if (!next) reset();
      }}
    >
      <DialogContent className="sm:max-w-sm">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <MicrosoftMark />
            Connexion Microsoft
          </DialogTitle>
          <DialogDescription>Connecte ton compte Microsoft pour lancer Minecraft.</DialogDescription>
        </DialogHeader>

        {status === "idle" && (
          <Button onClick={startLogin} className="w-full gap-2">
            <MicrosoftMark />
            Se connecter avec Microsoft
          </Button>
        )}

        {status === "waiting" && (
          <div className="flex items-center justify-center gap-2 py-6 text-sm text-muted-foreground">
            <Loader2 className="size-4 animate-spin" aria-hidden="true" />
            Préparation de la connexion…
          </div>
        )}

        {status === "polling" && device && (
          <div className="space-y-3 text-center">
            <p className="text-sm text-muted-foreground">
              Entre ce code sur la page qui va s'ouvrir :
            </p>
            <p className="rounded-md bg-muted py-3 font-mono text-2xl font-semibold tracking-widest">
              {device.user_code}
            </p>
            <Button
              variant="outline"
              size="sm"
              className="w-full gap-2"
              onClick={() => openUrl(device.verification_uri)}
            >
              <ExternalLink className="size-3.5" aria-hidden="true" />
              Ouvrir la page de connexion
            </Button>
            <p className="flex items-center justify-center gap-2 text-xs text-muted-foreground">
              <Loader2 className="size-3 animate-spin" aria-hidden="true" />
              En attente de la validation…
            </p>
          </div>
        )}

        {status === "error" && (
          <div className="space-y-3">
            <p className="text-sm text-destructive">{error}</p>
            <Button variant="outline" className="w-full" onClick={reset}>
              Réessayer
            </Button>
          </div>
        )}
      </DialogContent>
    </Dialog>
  );
}
