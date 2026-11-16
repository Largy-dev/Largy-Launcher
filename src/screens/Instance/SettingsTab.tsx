import { useEffect, useState } from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { useSearchParams } from "react-router";
import { Check, Coffee, FolderOpen, Gamepad2, Info, Loader2, MemoryStick } from "lucide-react";

import { InstanceActions } from "@/components/instance/InstanceActions";
import { MemorySlider } from "@/components/MemorySlider";
import {
  JvmArgWarnings,
  SettingRow,
  SettingSection,
  SettingsLayout,
  UnsavedChanges,
  type SettingsTab,
} from "@/components/settings/SettingsKit";
import { JavaPicker } from "@/components/settings/JavaPicker";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Switch } from "@/components/ui/switch";
import { Textarea } from "@/components/ui/textarea";
import { useModCount, useRamAdvice, useSystemMemory } from "@/hooks/useInstanceInfo";
import { useSettings } from "@/hooks/useSettings";
import { formatDuration, formatGb, formatRelative } from "@/lib/format";
import { jvmArgIssues, parseJvmArgs } from "@/lib/jvmArgs";
import { JVM_PRESETS, isPresetActive, togglePreset } from "@/lib/jvmPresets";
import { notify } from "@/lib/notify";
import { minecraftMinor } from "@/lib/ramAdvice";
import { cn } from "@/lib/utils";
import { errorMessage, instancesApi, type Instance } from "@/services/tauri";
import { JvmPresetsSection } from "@/screens/InstanceSettings/JvmPresetsSection";
import { OrganizationSection } from "@/screens/InstanceSettings/OrganizationSection";
import { SnapshotsSection } from "@/screens/InstanceSettings/SnapshotsSection";

type TabId = "general" | "game" | "memory" | "java";

const TABS: SettingsTab<TabId>[] = [
  { id: "general", label: "Général", icon: Info, description: "Nom, emplacement, outils et statistiques." },
  { id: "game", label: "Jeu", icon: Gamepad2, description: "Fenêtre du jeu et connexion directe à un serveur." },
  { id: "memory", label: "Mémoire", icon: MemoryStick, description: "La RAM allouée au jeu, avec un conseil adapté." },
  { id: "java", label: "Java", icon: Coffee, description: "Version de Java, optimisations et arguments de la JVM." },
];

interface Draft {
  name: string;
  minMb: number | null;
  maxMb: number | null;
  jvmArgs: string;
  javaPath: string | null;
  width: number | null;
  height: number | null;
  fullscreen: boolean;
  server: string;
  ownOptions: boolean;
}

function draftOf(instance: Instance): Draft {
  return {
    name: instance.name,
    minMb: instance.min_memory_mb,
    maxMb: instance.max_memory_mb,
    jvmArgs: instance.extra_jvm_args.join(" "),
    javaPath: instance.java_path,
    width: instance.window_width,
    height: instance.window_height,
    fullscreen: instance.fullscreen,
    server: instance.auto_join_server ?? "",
    ownOptions: instance.skip_options_sync,
  };
}

const numberOrNull = (raw: string) => (raw.trim() === "" ? null : Math.max(0, Math.round(Number(raw)) || 0));

/** Everything that changes how this instance launches, plus its maintenance tools. */
export function SettingsTab({ instance }: { instance: Instance }) {
  const instanceId = instance.id;
  const queryClient = useQueryClient();
  const [params, setParams] = useSearchParams();
  const tab = (TABS.find((t) => t.id === params.get("section"))?.id ?? "general") as TabId;

  const { data: settings } = useSettings();
  const { data: memory } = useSystemMemory();
  const modCount = useModCount(instance);
  const ram = useRamAdvice(instance, modCount);
  const [draft, setDraft] = useState<Draft | null>(null);

  useEffect(() => {
    // Seeds the local draft from the instance, and again whenever it's
    // refetched; doesn't cascade since `instance` only changes on refetch.
    // eslint-disable-next-line react-hooks/set-state-in-effect
    setDraft(draftOf(instance));
  }, [instance]);

  const saved = draftOf(instance);
  const dirty = !!draft && JSON.stringify(draft) !== JSON.stringify(saved);

  const saveMutation = useMutation({
    mutationFn: async (next: Draft) => {
      if (next.name.trim() !== saved.name) await instancesApi.rename(instanceId, next.name);
      return instancesApi.updateSettings(instanceId, {
        min_memory_mb: next.minMb,
        max_memory_mb: next.maxMb,
        extra_jvm_args: parseJvmArgs(next.jvmArgs),
        java_path: next.javaPath,
        window_width: next.width,
        window_height: next.height,
        fullscreen: next.fullscreen,
        auto_join_server: next.server.trim() || null,
        skip_options_sync: next.ownOptions,
      });
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["instance", instanceId] });
      queryClient.invalidateQueries({ queryKey: ["instances"] });
      notify.success({ title: "Réglages de l'instance enregistrés", history: false });
    },
    onError: (e) => notify.error({ title: "Enregistrement impossible", message: errorMessage(e) }),
  });

  if (!draft) {
    return (
      <div className="flex flex-1 items-center justify-center">
        <Loader2 className="size-5 animate-spin text-muted-foreground" aria-hidden="true" />
      </div>
    );
  }

  const patch = (p: Partial<Draft>) => setDraft((d) => (d ? { ...d, ...p } : d));
  const defaultMax = settings?.default_max_memory_mb ?? 4096;
  const args = parseJvmArgs(draft.jvmArgs);
  const minor = minecraftMinor(instance.minecraft_version);

  return (
    <div className="flex flex-col">
      <SettingsLayout
        tabs={TABS}
        active={tab}
        onChange={(next) =>
          setParams(
            (p) => {
              p.set("section", next);
              return p;
            },
            { replace: true },
          )
        }
      >
        {tab === "general" && (
          <>
            <SettingSection>
              <SettingRow
                label="Nom"
                control={
                  <Input
                    className="w-72"
                    maxLength={64}
                    value={draft.name}
                    onChange={(e) => patch({ name: e.target.value })}
                  />
                }
              />
              <SettingRow
                label="Dossier"
                description={<span className="font-mono break-all">{instance.directory}</span>}
                control={
                  <Button
                    variant="outline"
                    size="sm"
                    className="gap-1.5"
                    onClick={() => instancesApi.openFolder(instanceId)}
                  >
                    <FolderOpen aria-hidden="true" />
                    Ouvrir
                  </Button>
                }
              />
            </SettingSection>
            <SettingSection title="Statistiques">
              <div className="grid grid-cols-3 divide-x divide-border/60">
                {[
                  {
                    label: "Temps de jeu",
                    value: instance.play_time_seconds ? formatDuration(instance.play_time_seconds) : "—",
                  },
                  {
                    label: "Dernière partie",
                    value: instance.last_played_at ? formatRelative(instance.last_played_at) : "Jamais",
                  },
                  { label: "Créée", value: instance.created_at ? formatRelative(instance.created_at) : "—" },
                ].map((stat) => (
                  <div key={stat.label} className="px-4 py-3">
                    <p className="text-base font-bold">{stat.value}</p>
                    <p className="text-xs text-muted-foreground">{stat.label}</p>
                  </div>
                ))}
              </div>
            </SettingSection>
            <OrganizationSection instance={instance} />
            <SnapshotsSection instance={instance} />
            <InstanceActions instance={instance} />
          </>
        )}

        {tab === "game" && (
          <SettingSection>
            <SettingRow
              label="Taille de la fenêtre"
              description="Vide = taille par défaut de Minecraft."
              control={
                <div className="flex items-center gap-2">
                  <Input
                    type="number"
                    className="w-24"
                    min={320}
                    placeholder="Largeur"
                    aria-label="Largeur"
                    value={draft.width ?? ""}
                    onChange={(e) => patch({ width: numberOrNull(e.target.value) })}
                  />
                  <span className="text-muted-foreground">×</span>
                  <Input
                    type="number"
                    className="w-24"
                    min={240}
                    placeholder="Hauteur"
                    aria-label="Hauteur"
                    value={draft.height ?? ""}
                    onChange={(e) => patch({ height: numberOrNull(e.target.value) })}
                  />
                </div>
              }
            />
            <SettingRow
              label="Plein écran"
              description="Démarre le jeu directement en plein écran."
              control={<Switch checked={draft.fullscreen} onCheckedChange={(fullscreen) => patch({ fullscreen })} />}
            />
            <SettingRow
              label="Rejoindre un serveur au lancement"
              description="Adresse du serveur (ex. play.exemple.fr ou play.exemple.fr:25566). Vide = menu principal."
              control={
                <Input
                  className="w-72"
                  placeholder="play.exemple.fr"
                  value={draft.server}
                  onChange={(e) => patch({ server: e.target.value })}
                />
              }
            />
            {settings?.sync_game_options && (
              <SettingRow
                label="Options propres à cette instance"
                description="Les touches, la sensibilité, le FOV et les volumes sont partagés entre instances (Paramètres › Jeu & Java). Active ceci pour que cette instance garde les siens."
                control={<Switch checked={draft.ownOptions} onCheckedChange={(ownOptions) => patch({ ownOptions })} />}
              />
            )}
          </SettingSection>
        )}

        {tab === "memory" && (
          <SettingSection>
            <SettingRow
              label="Utiliser la valeur par défaut"
              description={`Suit le réglage global (${formatGb(defaultMax)}).`}
              control={
                <Switch
                  checked={draft.maxMb === null}
                  onCheckedChange={(useDefault) =>
                    patch({ maxMb: useDefault ? null : (ram?.advice.recommendedMb ?? defaultMax) })
                  }
                />
              }
            />
            <SettingRow
              label="RAM maximale"
              stacked
              control={
                <div className={cn(draft.maxMb === null && "pointer-events-none opacity-60")}>
                  <MemorySlider
                    valueMb={draft.maxMb ?? defaultMax}
                    onChangeMb={(v) => patch({ maxMb: v })}
                    maxMb={memory?.total_mb ?? 16384}
                    advice={ram?.advice}
                  />
                </div>
              }
            />
            <SettingRow
              label="RAM minimale (Mo)"
              description={`Vide = valeur par défaut (${settings?.default_min_memory_mb ?? 1024} Mo).`}
              control={
                <Input
                  type="number"
                  className="w-28"
                  min={256}
                  step={256}
                  placeholder={String(settings?.default_min_memory_mb ?? 1024)}
                  value={draft.minMb ?? ""}
                  onChange={(e) => patch({ minMb: e.target.value ? Number(e.target.value) : null })}
                />
              }
            />
          </SettingSection>
        )}

        {tab === "java" && (
          <>
            <SettingSection title="Version de Java">
              <SettingRow
                label="Java utilisé"
                description="Automatique = le Java recommandé pour cette version, téléchargé par le launcher."
                control={
                  <JavaPicker
                    value={draft.javaPath}
                    onChange={(javaPath) => patch({ javaPath })}
                    autoLabel={settings?.java_path_override ? "Réglage global" : "Automatique (recommandé)"}
                  />
                }
              />
            </SettingSection>
            <SettingSection title="Optimisations en un clic">
              {JVM_PRESETS.map((preset) => {
                const active = isPresetActive(args, preset);
                const unsupported = preset.minMinecraftMinor !== undefined && minor < preset.minMinecraftMinor;
                return (
                  <SettingRow
                    key={preset.id}
                    label={preset.label}
                    description={
                      unsupported ? `${preset.description} Indisponible pour cette version.` : preset.description
                    }
                    control={
                      <Button
                        size="sm"
                        variant={active ? "default" : "outline"}
                        disabled={unsupported && !active}
                        className="w-24 gap-1.5"
                        onClick={() => patch({ jvmArgs: togglePreset(args, preset).join(" ") })}
                      >
                        {active && <Check aria-hidden="true" />}
                        {active ? "Activé" : "Activer"}
                      </Button>
                    }
                  />
                );
              })}
            </SettingSection>
            <JvmPresetsSection
              minMb={draft.minMb}
              maxMb={draft.maxMb ?? defaultMax}
              args={args}
              onApply={(preset) => patch({ minMb: preset.minMb, maxMb: preset.maxMb, jvmArgs: preset.jvmArgs })}
            />
            <SettingSection title="Arguments personnalisés">
              <div className="p-4">
                <Textarea
                  className="min-h-28 font-mono text-xs"
                  placeholder="-Dfoo=bar"
                  value={draft.jvmArgs}
                  onChange={(e) => patch({ jvmArgs: e.target.value })}
                />
                <p className="mt-2 text-xs text-muted-foreground">
                  Ajoutés après les arguments par défaut du launcher. La mémoire se règle dans l'onglet Mémoire.
                </p>
                <JvmArgWarnings
                  className="mt-2"
                  issues={jvmArgIssues([...(settings?.default_jvm_args ?? []), ...args], minor)}
                />
              </div>
            </SettingSection>
          </>
        )}
      </SettingsLayout>

      <UnsavedChanges
        dirty={dirty}
        saving={saveMutation.isPending}
        onSave={() => saveMutation.mutateAsync(draft)}
        onReset={() => setDraft(draftOf(instance))}
        leaves={(current, next) =>
          current.pathname !== next.pathname || new URLSearchParams(next.search).get("tab") !== "settings"
        }
      />
    </div>
  );
}
