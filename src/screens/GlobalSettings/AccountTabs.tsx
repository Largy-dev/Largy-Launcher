import { useState } from "react";
import { BellRing, LogIn, LogOut } from "lucide-react";

import { SettingRow, SettingSection } from "@/components/settings/SettingsKit";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Switch } from "@/components/ui/switch";
import { notify } from "@/lib/notify";
import { LoginDialog } from "@/screens/Login/LoginScreen";
import { auth, type GlobalSettings } from "@/services/tauri";
import { useAppStore } from "@/store/appStore";
import { usePreferences, type NotificationPreferences } from "@/store/preferencesStore";

interface FormTabProps {
  form: GlobalSettings;
  update: <K extends keyof GlobalSettings>(key: K, value: GlobalSettings[K]) => void;
}

export function AccountTab({ form, update }: FormTabProps) {
  const account = useAppStore((s) => s.account);
  const setAccount = useAppStore((s) => s.setAccount);
  const [loginOpen, setLoginOpen] = useState(false);

  return (
    <>
      <SettingSection title="Compte Microsoft">
        {account ? (
          <div className="flex items-center gap-5 p-4">
            <img
              src={`https://mc-heads.net/body/${account.profile.id}/120`}
              alt={`Skin de ${account.profile.name}`}
              className="h-32 drop-shadow-xl [image-rendering:pixelated]"
            />
            <div className="flex-1 space-y-1">
              <p className="text-xl font-bold">{account.profile.name}</p>
              <p className="text-sm text-success">Connecté · serveurs officiels disponibles</p>
              <p className="font-mono text-[0.7rem] text-muted-foreground">{account.profile.id}</p>
            </div>
            <Button
              variant="outline"
              className="gap-1.5"
              onClick={async () => {
                await auth.logout();
                setAccount(null);
              }}
            >
              <LogOut aria-hidden="true" />
              Se déconnecter
            </Button>
          </div>
        ) : (
          <SettingRow
            label="Aucun compte connecté"
            description="Connecte ton compte Microsoft pour jouer en ligne avec ton skin."
            control={
              <Button onClick={() => setLoginOpen(true)} className="bg-gradient-brand gap-1.5">
                <LogIn aria-hidden="true" />
                Se connecter
              </Button>
            }
          />
        )}
      </SettingSection>

      <SettingSection title="Mode Hors-ligne">
        <SettingRow
          label="Jouer sans compte Microsoft"
          description="Profil local — solo ou serveurs en mode hors-ligne uniquement, pas les serveurs officiels."
          control={<Switch checked={form.offline_mode} onCheckedChange={(v) => update("offline_mode", v)} />}
        />
        {form.offline_mode && (
          <SettingRow
            label="Pseudo hors-ligne"
            description="16 caractères max. Toujours le même UUID pour ce pseudo."
            control={
              <Input
                className="w-56"
                placeholder="Steve"
                maxLength={16}
                value={form.offline_username}
                onChange={(e) => update("offline_username", e.target.value)}
              />
            }
          />
        )}
      </SettingSection>
      <LoginDialog open={loginOpen} onOpenChange={setLoginOpen} />
    </>
  );
}

const NOTIFICATION_ROWS: {
  key: Exclude<keyof NotificationPreferences, "native">;
  label: string;
  description: string;
}[] = [
  { key: "crash", label: "Crash du jeu", description: "Avec le diagnostic et un raccourci vers les logs." },
  { key: "gameExit", label: "Fin de session", description: "Quand le jeu se ferme normalement, avec la durée jouée." },
  { key: "installDone", label: "Installation terminée", description: "Modpack installé ou mis à jour." },
  { key: "updates", label: "Mise à jour du launcher", description: "Quand une nouvelle version est disponible." },
];

export function NotificationsTab() {
  const notifications = usePreferences((s) => s.notifications);
  const setNotifications = usePreferences((s) => s.setNotifications);

  return (
    <>
      <SettingSection title="Notifications Windows">
        <SettingRow
          label="Notifications natives"
          description="Affichées par Windows quand le launcher est en arrière-plan (pendant que tu joues, par exemple)."
          control={<Switch checked={notifications.native} onCheckedChange={(native) => setNotifications({ native })} />}
        />
        {NOTIFICATION_ROWS.map(({ key, label, description }) => (
          <SettingRow
            key={key}
            label={label}
            description={description}
            control={
              <Switch
                disabled={!notifications.native}
                checked={notifications[key]}
                onCheckedChange={(v) => setNotifications({ [key]: v })}
              />
            }
          />
        ))}
      </SettingSection>
      <div className="flex justify-end">
        <Button
          variant="outline"
          size="sm"
          className="gap-1.5"
          onClick={() =>
            notify.success({
              title: "Tout fonctionne !",
              message: "Voici à quoi ressemble une notification du launcher.",
              history: false,
            })
          }
        >
          <BellRing aria-hidden="true" />
          Tester
        </Button>
      </div>
    </>
  );
}
