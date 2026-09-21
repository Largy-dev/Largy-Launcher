import { useEffect, useState, type ReactNode } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Loader2 } from "lucide-react";
import { toast } from "sonner";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Switch } from "@/components/ui/switch";
import { PageHeader } from "@/components/PageHeader";
import { useAppVersion } from "@/hooks/useAppVersion";
import { errorMessage, settingsApi, type GlobalSettings } from "@/services/tauri";

interface SettingRowProps {
  label: string;
  description: string;
  control: ReactNode;
}

function SettingRow({ label, description, control }: SettingRowProps) {
  return (
    <div className="flex items-center justify-between gap-6 px-4 py-3">
      <div>
        <p className="text-sm font-medium">{label}</p>
        <p className="text-sm text-muted-foreground">{description}</p>
      </div>
      {control}
    </div>
  );
}

function SettingSection({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section className="mb-6">
      <h3 className="mb-2 text-xs font-medium uppercase tracking-wide text-muted-foreground">
        {title}
      </h3>
      <div className="divide-y divide-border rounded-lg border border-border">{children}</div>
    </section>
  );
}

export function GlobalSettingsScreen() {
  const version = useAppVersion();
  const queryClient = useQueryClient();
  const { data: settings, isLoading } = useQuery({ queryKey: ["settings"], queryFn: settingsApi.get });
  const [form, setForm] = useState<GlobalSettings | null>(null);

  useEffect(() => {
    if (settings) setForm(settings);
  }, [settings]);

  const saveMutation = useMutation({
    mutationFn: (next: GlobalSettings) => settingsApi.update(next),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["settings"] });
      toast.success("Paramètres enregistrés");
    },
    onError: (e) => toast.error(errorMessage(e)),
  });

  if (isLoading || !form) {
    return (
      <div className="flex flex-1 items-center justify-center">
        <Loader2 className="size-5 animate-spin text-muted-foreground" aria-hidden="true" />
      </div>
    );
  }

  function update<K extends keyof GlobalSettings>(key: K, value: GlobalSettings[K]) {
    setForm((f) => (f ? { ...f, [key]: value } : f));
  }

  return (
    <div className="flex flex-1 flex-col overflow-y-auto">
      <PageHeader
        title="Paramètres"
        description="Préférences générales du launcher."
        action={
          <Button
            onClick={() => form && saveMutation.mutate(form)}
            disabled={saveMutation.isPending}
            className="gap-1.5"
          >
            {saveMutation.isPending && <Loader2 className="size-3.5 animate-spin" aria-hidden="true" />}
            Enregistrer
          </Button>
        }
      />

      <SettingSection title="Comptes & API">
        <SettingRow
          label="Clé API CurseForge"
          description="Gratuite, depuis console.curseforge.com — active l'onglet CurseForge dans Modpacks (voir le README)."
          control={
            <Input
              className="w-64"
              type="password"
              placeholder="Clé API"
              value={form.curseforge_api_key}
              onChange={(e) => update("curseforge_api_key", e.target.value)}
            />
          }
        />
      </SettingSection>

      <SettingSection title="Mode Hors-ligne">
        <SettingRow
          label="Jouer sans compte Microsoft"
          description="Utilise un profil local à la place — fonctionne en solo ou sur un serveur configuré en mode hors-ligne uniquement, pas sur les serveurs officiels."
          control={<Switch checked={form.offline_mode} onCheckedChange={(v) => update("offline_mode", v)} />}
        />
        {form.offline_mode && (
          <SettingRow
            label="Pseudo hors-ligne"
            description="16 caractères maximum. Toujours le même UUID pour ce pseudo, comme sur un vrai serveur hors-ligne."
            control={
              <Input
                className="w-64"
                placeholder="Steve"
                maxLength={16}
                value={form.offline_username}
                onChange={(e) => update("offline_username", e.target.value)}
              />
            }
          />
        )}
      </SettingSection>

      <SettingSection title="Java & performance">
        <SettingRow
          label="Mémoire min. par défaut"
          description="Utilisée si une instance n'a pas de réglage propre (Mo)."
          control={
            <Input
              type="number"
              className="w-28"
              value={form.default_min_memory_mb}
              onChange={(e) => update("default_min_memory_mb", Number(e.target.value))}
            />
          }
        />
        <SettingRow
          label="Mémoire max. par défaut"
          description="Utilisée si une instance n'a pas de réglage propre (Mo)."
          control={
            <Input
              type="number"
              className="w-28"
              value={form.default_max_memory_mb}
              onChange={(e) => update("default_max_memory_mb", Number(e.target.value))}
            />
          }
        />
        <SettingRow
          label="Chemin Java personnalisé"
          description="Laisse vide pour que le launcher télécharge et gère Java automatiquement."
          control={
            <Input
              className="w-64"
              placeholder="Automatique"
              value={form.java_path_override ?? ""}
              onChange={(e) => update("java_path_override", e.target.value || null)}
            />
          }
        />
        <SettingRow
          label="Arguments JVM par défaut"
          description="Appliqués à toutes les instances, en plus de leurs réglages propres."
          control={
            <Input
              className="w-64"
              placeholder="Aucun"
              value={form.default_jvm_args.join(" ")}
              onChange={(e) => update("default_jvm_args", e.target.value.split(/\s+/).filter(Boolean))}
            />
          }
        />
      </SettingSection>

      <SettingSection title="À propos">
        <SettingRow
          label="Version"
          description="Largy Launcher"
          control={
            <span className="font-mono text-sm text-muted-foreground">{version ? `v${version}` : "—"}</span>
          }
        />
      </SettingSection>
    </div>
  );
}
