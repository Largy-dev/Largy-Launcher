import { useState } from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { ExternalLink, Loader2 } from "lucide-react";
import { openUrl } from "@tauri-apps/plugin-opener";

import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { useSettings } from "@/hooks/useSettings";
import { notify } from "@/lib/notify";
import { auth, errorMessage, isCancelled, settingsApi, type DeviceCodeInfo } from "@/services/tauri";
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

type LoginStatus = "idle" | "waiting" | "polling" | "confirm-offline" | "error";

interface LoginDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

export function LoginDialog({ open, onOpenChange }: LoginDialogProps) {
  const setAccount = useAppStore((s) => s.setAccount);
  const [device, setDevice] = useState<DeviceCodeInfo | null>(null);
  const [status, setStatus] = useState<LoginStatus>("idle");
  const [error, setError] = useState<string | null>(null);
  const { data: settings } = useSettings();
  const queryClient = useQueryClient();

  const disableOfflineMutation = useMutation({
    mutationFn: () => settingsApi.update({ ...settings!, offline_mode: false }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["settings"] });
      notify.success({ title: "Mode hors-ligne désactivé", history: false });
      onOpenChange(false);
      reset();
    },
    onError: (e) => notify.error({ title: "Impossible de modifier les paramètres", message: errorMessage(e) }),
  });

  function reset() {
    setDevice(null);
    setStatus("idle");
    setError(null);
  }

  function finishLogin() {
    onOpenChange(false);
    reset();
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
      queryClient.invalidateQueries({ queryKey: ["accounts"] });
      notify.success({ title: `Bienvenue, ${session.profile.name} !`, message: "Compte Microsoft connecté." });
      if (settings?.offline_mode) {
        setStatus("confirm-offline");
      } else {
        finishLogin();
      }
    } catch (e) {
      if (isCancelled(e)) return;
      setError(errorMessage(e));
      setStatus("error");
    }
  }

  async function copyAndOpen(info: DeviceCodeInfo) {
    try {
      await navigator.clipboard.writeText(info.user_code);
      notify.success({ title: "Code copié", message: "Colle-le sur la page Microsoft.", history: false });
    } catch {
      // Clipboard unavailable: the code stays readable on screen.
    }
    await openUrl(info.verification_uri);
  }

  return (
    <Dialog
      open={open}
      onOpenChange={(next) => {
        onOpenChange(next);
        if (!next) {
          if (status === "polling" || status === "waiting") auth.cancelLogin().catch(() => {});
          reset();
        }
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
            <p className="text-sm text-muted-foreground">Entre ce code sur la page qui va s'ouvrir :</p>
            <p className="rounded-md bg-muted py-3 font-mono text-2xl font-semibold tracking-widest">
              {device.user_code}
            </p>
            <Button variant="outline" size="sm" className="w-full gap-2" onClick={() => copyAndOpen(device)}>
              <ExternalLink className="size-3.5" aria-hidden="true" />
              Copier le code et ouvrir la page
            </Button>
            <p className="flex items-center justify-center gap-2 text-xs text-muted-foreground">
              <Loader2 className="size-3 animate-spin" aria-hidden="true" />
              En attente de la validation…
            </p>
          </div>
        )}

        {status === "confirm-offline" && (
          <div className="space-y-3">
            <p className="text-sm text-muted-foreground">
              Tu es connecté avec ton compte Microsoft, mais le Mode Hors-ligne est encore activé — tant qu'il l'est, le
              jeu se lance avec le profil local, pas avec ce compte. Le désactiver ?
            </p>
            <div className="flex gap-2">
              <Button variant="outline" className="flex-1" onClick={finishLogin}>
                Garder le mode hors-ligne
              </Button>
              <Button
                className="flex-1 gap-1.5"
                onClick={() => disableOfflineMutation.mutate()}
                disabled={disableOfflineMutation.isPending}
              >
                {disableOfflineMutation.isPending && <Loader2 className="size-3.5 animate-spin" aria-hidden="true" />}
                Désactiver
              </Button>
            </div>
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
