import { useEffect, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { MonitorDown, Power } from "lucide-react";

import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Switch } from "@/components/ui/switch";
import { closeApp, onCloseRequested, type CloseBehavior } from "@/services/tauri";
import { useAppStore } from "@/store/appStore";

/** Asked the first time the window's close button is pressed: reduce to the tray or quit. */
export function CloseDialog() {
  const queryClient = useQueryClient();
  const [open, setOpen] = useState(false);
  const [remember, setRemember] = useState(true);
  const gameRunning = useAppStore((s) => Object.values(s.runtime).some((r) => r.running));

  useEffect(() => {
    const unlisten = onCloseRequested(() => setOpen(true));
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  async function choose(action: Exclude<CloseBehavior, "ask">) {
    setOpen(false);
    await closeApp(action, remember);
    if (remember) queryClient.invalidateQueries({ queryKey: ["settings"] });
  }

  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>Fermer Largy Launcher ?</DialogTitle>
          <DialogDescription>
            Réduit, le launcher reste disponible dans la zone de notification (près de l'horloge) : il continue de
            compter ton temps de jeu et de détecter les crashs.
            {gameRunning && " Un jeu est en cours : si tu quittes, son temps de jeu ne sera pas enregistré."}
          </DialogDescription>
        </DialogHeader>
        <label className="flex items-center gap-2 text-sm">
          <Switch checked={remember} onCheckedChange={setRemember} />
          Se souvenir de mon choix (modifiable dans Paramètres › Jeu & Java)
        </label>
        <DialogFooter>
          <Button variant="outline" className="gap-1.5" onClick={() => choose("quit")}>
            <Power aria-hidden="true" />
            Quitter
          </Button>
          <Button className="gap-1.5" onClick={() => choose("tray")}>
            <MonitorDown aria-hidden="true" />
            Réduire
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
