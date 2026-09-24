import { useEffect, useMemo, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useSearchParams } from "react-router";
import { openUrl } from "@tauri-apps/plugin-opener";
import {
  Bell,
  CheckCircle2,
  Coffee,
  ExternalLink,
  FileText,
  Info,
  Loader2,
  Palette,
  Trash2,
  UserRound,
  Wrench,
} from "lucide-react";

import { MemorySlider } from "@/components/MemorySlider";
import { PageHeader } from "@/components/PageHeader";
import { JavaPicker } from "@/components/settings/JavaPicker";
import {
  ChoiceGroup,
  SettingRow,
  SettingSection,
  SettingsLayout,
  UnsavedChanges,
  type SettingsTab,
} from "@/components/settings/SettingsKit";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Switch } from "@/components/ui/switch";
import { useAppVersion } from "@/hooks/useAppVersion";
import { useSystemMemory } from "@/hooks/useInstanceInfo";
import { useSettings } from "@/hooks/useSettings";
import { formatBytes, formatGb } from "@/lib/format";
import { parseJvmArgs } from "@/lib/jvmArgs";
import { notify } from "@/lib/notify";
import { adviseRam } from "@/lib/ramAdvice";
import { checkForAppUpdate, installAppUpdate } from "@/lib/updater";
import {
  errorMessage,
  openLauncherLogs,
  settingsApi,
  type CloseBehavior,
  type GlobalSettings,
  type LauncherBehavior,
} from "@/services/tauri";

const CLOSE_BEHAVIORS: { value: CloseBehavior; label: string }[] = [
  { value: "ask", label: "Demander" },
  { value: "tray", label: "Réduire" },
  { value: "quit", label: "Quitter" },
];

const BEHAVIORS: { value: LauncherBehavior; label: string }[] = [
  { value: "keep_open", label: "Rester ouvert" },
  { value: "minimize", label: "Réduire" },
  { value: "hide", label: "Masquer" },
];

import { AccountTab, NotificationsTab } from "./AccountTabs";
import { AppearanceTab } from "./AppearanceTab";

type TabId = "appearance" | "game" | "account" | "notifications" | "advanced" | "about";

const TABS: SettingsTab<TabId>[] = [
  {
    id: "appearance",
    label: "Apparence",
    icon: Palette,
    description: "Thème, couleurs, fond et animations — appliqués instantanément.",
  },
  {
    id: "game",
    label: "Jeu & Java",
    icon: Coffee,
    description: "Mémoire et Java utilisés par défaut par toutes les instances.",
  },
  { id: "account", label: "Compte", icon: UserRound, description: "Ton compte Microsoft ou un profil hors-ligne." },
  { id: "notifications", label: "Notifications", icon: Bell, description: "Ce qui mérite de te déranger." },
  { id: "advanced", label: "Avancé", icon: Wrench, description: "Clés d'API et options pour utilisateurs avertis." },
  { id: "about", label: "À propos", icon: Info, description: "Version et mises à jour." },
];

const REPO_URL = "https://github.com/Largy-dev/Largy-Launcher";

function isTab(value: string | null): value is TabId {
  return TABS.some((t) => t.id === value);
}

function AboutTab() {
  const version = useAppVersion();
  const [percent, setPercent] = useState<number | null>(null);
  const check = useMutation({
    mutationFn: checkForAppUpdate,
    onError: (e) => notify.error({ title: "Vérification impossible", message: errorMessage(e), history: false }),
  });

  async function install() {
    if (!check.data) return;
    setPercent(0);
    try {
      await installAppUpdate(check.data, setPercent);
    } catch (e) {
      notify.error({ title: "Mise à jour impossible", message: errorMessage(e) });
      setPercent(null);
    }
  }

  return (
    <SettingSection>
      <div className="flex items-center gap-4 p-5">
        <div className="bg-gradient-brand shadow-glow flex size-14 items-center justify-center rounded-2xl text-2xl font-black text-primary-foreground">
          L
        </div>
        <div className="flex-1">
          <p className="text-lg font-bold">Largy Launcher</p>
          <p className="font-mono text-sm text-muted-foreground">{version ? `v${version}` : "—"}</p>
        </div>
        <Button variant="outline" size="sm" className="gap-1.5" onClick={() => openUrl(`${REPO_URL}/releases`)}>
          <ExternalLink aria-hidden="true" />
          Nouveautés
        </Button>
        <Button size="sm" onClick={() => check.mutate()} disabled={check.isPending} className="gap-1.5">
          {check.isPending && <Loader2 className="animate-spin" aria-hidden="true" />}
          Vérifier les mises à jour
        </Button>
      </div>
      {check.isSuccess && (
        <SettingRow
          label={check.data ? `Version ${check.data.version} disponible` : "Tu es à jour"}
          description={
            check.data
              ? percent !== null
                ? `Téléchargement… ${percent}%`
                : `Tu utilises la v${check.data.currentVersion}.`
              : "Aucune nouvelle version pour le moment."
          }
          control={
            check.data ? (
              <Button size="sm" onClick={install} disabled={percent !== null} className="bg-gradient-brand gap-1.5">
                {percent !== null && <Loader2 className="animate-spin" aria-hidden="true" />}
                Mettre à jour
              </Button>
            ) : (
              <CheckCircle2 className="size-5 text-success" aria-hidden="true" />
            )
          }
        />
      )}
    </SettingSection>
  );
}

function InstallerCacheRow() {
  const queryClient = useQueryClient();
  const { data: size } = useQuery({ queryKey: ["installer-cache-size"], queryFn: settingsApi.installerCacheSize });
  const clear = useMutation({
    mutationFn: settingsApi.clearInstallerCache,
    onSuccess: (freed) => {
      queryClient.setQueryData(["installer-cache-size"], 0);
      notify.success({ title: `${formatBytes(freed)} libérés`, history: false });
    },
    onError: (e) => notify.error({ title: "Nettoyage impossible", message: errorMessage(e) }),
  });

  return (
    <SettingRow
      label="Cache des installateurs"
      description="Fichiers Forge/NeoForge téléchargés lors des installations — retéléchargés automatiquement au besoin."
      control={
        <Button
          variant="outline"
          size="sm"
          className="gap-1.5"
          disabled={!size || clear.isPending}
          onClick={() => clear.mutate()}
        >
          {clear.isPending ? <Loader2 className="animate-spin" aria-hidden="true" /> : <Trash2 aria-hidden="true" />}
          Vider {size ? `(${formatBytes(size)})` : ""}
        </Button>
      }
    />
  );
}

export function GlobalSettingsScreen() {
  const queryClient = useQueryClient();
  const [params, setParams] = useSearchParams();
  const tab: TabId = isTab(params.get("tab")) ? (params.get("tab") as TabId) : "appearance";
  const { data: settings } = useSettings();
  const { data: memory } = useSystemMemory();
  const [form, setForm] = useState<GlobalSettings | null>(null);

  useEffect(() => {
    // Seeds the local draft from the query once it loads; doesn't cascade
    // since `settings` only changes on refetch.
    // eslint-disable-next-line react-hooks/set-state-in-effect
    if (settings) setForm(settings);
  }, [settings]);

  const dirty = !!form && !!settings && JSON.stringify(form) !== JSON.stringify(settings);
  const saveMutation = useMutation({
    mutationFn: (next: GlobalSettings) => settingsApi.update(next),
    onSuccess: (saved) => {
      queryClient.setQueryData(["settings"], saved);
      setForm(saved);
      notify.success({ title: "Paramètres enregistrés", history: false });
    },
    onError: (e) => notify.error({ title: "Enregistrement impossible", message: errorMessage(e) }),
  });

  // The global default applies to any instance, so advise for a typical mid-sized modpack.
  const advice = useMemo(
    () =>
      adviseRam({
        loader: "neoforge",
        modCount: 100,
        minecraftVersion: "1.20.1",
        isModpack: true,
        systemTotalMb: memory?.total_mb ?? null,
      }),
    [memory?.total_mb],
  );

  function update<K extends keyof GlobalSettings>(key: K, value: GlobalSettings[K]) {
    setForm((f) => (f ? { ...f, [key]: value } : f));
  }

  return (
    <div className="flex flex-1 flex-col">
      <PageHeader eyebrow="Réglages" title="Paramètres" description="Personnalise le launcher à ton goût." />

      <SettingsLayout tabs={TABS} active={tab} onChange={(next) => setParams({ tab: next }, { replace: true })}>
        {tab === "appearance" && <AppearanceTab />}
        {tab === "notifications" && <NotificationsTab />}
        {tab === "about" && <AboutTab />}
        {!form && (tab === "game" || tab === "account" || tab === "advanced") && (
          <Loader2 className="mx-auto size-5 animate-spin text-muted-foreground" aria-hidden="true" />
        )}
        {form && tab === "account" && <AccountTab form={form} update={update} />}
        {form && tab === "game" && (
          <>
            <SettingSection title="Mémoire par défaut">
              <SettingRow
                label="RAM maximale"
                description={
                  <>
                    Utilisée par les instances sans réglage propre. RAM de ce PC :{" "}
                    {memory ? `${formatGb(memory.total_mb)} (${formatGb(memory.available_mb)} libres)` : "—"}. Conseil
                    calculé pour un modpack moyen (~100 mods) ; chaque instance a son propre conseil.
                  </>
                }
                stacked
                control={
                  <MemorySlider
                    valueMb={form.default_max_memory_mb}
                    onChangeMb={(v) => update("default_max_memory_mb", v)}
                    maxMb={memory?.total_mb ?? 16384}
                    advice={advice}
                  />
                }
              />
              <SettingRow
                label="RAM minimale"
                description="Mémoire réservée dès le démarrage (Mo). Laisser bas convient à la plupart des cas."
                control={
                  <Input
                    type="number"
                    className="w-28"
                    min={256}
                    step={256}
                    value={form.default_min_memory_mb}
                    onChange={(e) => update("default_min_memory_mb", Math.max(0, Number(e.target.value) || 0))}
                    onBlur={() =>
                      update(
                        "default_min_memory_mb",
                        Math.min(Math.max(form.default_min_memory_mb, 256), form.default_max_memory_mb),
                      )
                    }
                  />
                }
              />
            </SettingSection>
            <SettingSection title="Java">
              <SettingRow
                label="Java par défaut"
                description="Automatique = le launcher télécharge le Java adapté à chaque version (recommandé)."
                control={
                  <JavaPicker
                    value={form.java_path_override}
                    onChange={(path) => update("java_path_override", path)}
                    autoLabel="Automatique (recommandé)"
                  />
                }
              />
              <SettingRow
                label="Arguments JVM par défaut"
                description="Ajoutés à toutes les instances, en plus de leurs réglages propres."
                control={
                  <Input
                    className="w-72 font-mono text-xs"
                    placeholder="Aucun"
                    value={form.default_jvm_args.join(" ")}
                    onChange={(e) => update("default_jvm_args", parseJvmArgs(e.target.value))}
                  />
                }
              />
            </SettingSection>
            <SettingSection title="Fenêtre du launcher">
              <SettingRow
                label="Bouton de fermeture"
                description="Réduire : le launcher reste dans la zone de notification (près de l'horloge) et continue de compter ton temps de jeu."
                control={
                  <ChoiceGroup
                    value={form.on_close}
                    options={CLOSE_BEHAVIORS}
                    onChange={(v) => update("on_close", v)}
                  />
                }
              />
              <SettingRow
                label="Quand le jeu démarre"
                description="Masquée : elle réapparaît automatiquement quand le jeu se ferme."
                control={
                  <ChoiceGroup
                    value={form.on_game_launch}
                    options={BEHAVIORS}
                    onChange={(v) => update("on_game_launch", v)}
                  />
                }
              />
            </SettingSection>
            <SettingSection title="Discord">
              <SettingRow
                label="Afficher ma partie sur Discord"
                description="Tes amis voient l'instance à laquelle tu joues et depuis combien de temps (Discord doit être ouvert)."
                control={
                  <Switch
                    checked={form.discord_rich_presence}
                    onCheckedChange={(v) => update("discord_rich_presence", v)}
                  />
                }
              />
            </SettingSection>
          </>
        )}
        {form && tab === "advanced" && (
          <SettingSection>
            <SettingRow
              label="Clé API CurseForge"
              description="Gratuite, depuis console.curseforge.com — active l'onglet CurseForge dans Modpacks."
              control={
                <Input
                  className="w-72"
                  type="password"
                  placeholder="Clé API"
                  value={form.curseforge_api_key}
                  onChange={(e) => update("curseforge_api_key", e.target.value)}
                />
              }
            />
            <SettingRow
              label="Journal du launcher"
              description="Utile pour signaler un problème : joins le fichier launcher.log."
              control={
                <Button variant="outline" size="sm" className="gap-1.5" onClick={() => openLauncherLogs()}>
                  <FileText aria-hidden="true" />
                  Ouvrir le dossier
                </Button>
              }
            />
            <InstallerCacheRow />
          </SettingSection>
        )}
      </SettingsLayout>

      <UnsavedChanges
        dirty={dirty}
        saving={saveMutation.isPending}
        onSave={() => (form ? saveMutation.mutateAsync(form) : undefined)}
        onReset={() => settings && setForm(settings)}
      />
    </div>
  );
}
