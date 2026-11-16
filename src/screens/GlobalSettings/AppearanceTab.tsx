import { Check, Image, Monitor, Moon, Paintbrush, Square, Sun, Sparkle } from "lucide-react";

import { ChoiceGroup, SettingRow, SettingSection } from "@/components/settings/SettingsKit";
import { Button } from "@/components/ui/button";
import { Slider } from "@/components/ui/slider";
import { ACCENTS, ACCENT_LABELS, ACCENT_SWATCHES, isHexColor, readableForeground } from "@/lib/theme";
import { cn } from "@/lib/utils";
import { usePreferences } from "@/store/preferencesStore";

const UI_SCALES = [90, 100, 110, 120];

/** Mini preview of the current theme: instance list, top tabs, title, Jouer and the tab panel. */
function ThemePreview() {
  return (
    <div className="relative h-40 overflow-hidden rounded-xl border border-border bg-background" aria-hidden="true">
      <div
        className="absolute inset-0"
        style={{
          backgroundImage: [
            "radial-gradient(22rem 8rem at 60% 110%, color-mix(in oklab, var(--sky-glow) 45%, transparent), transparent 70%)",
            "radial-gradient(20rem 10rem at 90% 0%, color-mix(in oklab, var(--accent-base) 30%, transparent), transparent 70%)",
            "linear-gradient(to bottom, var(--sky-top), var(--background) 70%)",
          ].join(","),
        }}
      />
      <div className="absolute inset-y-0 left-0 w-16 space-y-1.5 border-r border-sidebar-border bg-sidebar p-2">
        <div className="h-1.5 w-2/3 rounded bg-foreground/50" />
        {["--loader-fabric", "--loader-forge", "--loader-vanilla"].map((v, i) => (
          <div key={v} className={cn("flex items-center gap-1 rounded-md p-0.5", i === 0 && "bg-sidebar-accent")}>
            <div className="size-3.5 rounded" style={{ backgroundColor: `var(${v})` }} />
            <div className="h-1.5 flex-1 rounded bg-foreground/30" />
          </div>
        ))}
      </div>
      <div className="absolute inset-y-0 right-0 left-16 p-3">
        <div className="tile flex w-fit gap-1 p-0.5">
          <div className="h-2.5 w-8 rounded bg-foreground/20" />
          <div className="h-2.5 w-6 rounded bg-foreground/8" />
          <div className="h-2.5 w-6 rounded bg-foreground/8" />
        </div>
        <div className="mt-3 h-4 w-28 rounded bg-foreground/80" />
        <div className="mt-1.5 h-1.5 w-40 rounded bg-foreground/30" />
        <div className="mt-2.5 flex gap-1.5">
          <div className="bg-gradient-brand shadow-glow h-5 w-14 rounded-md" />
          <div className="tile h-5 w-12" />
          <div className="tile size-5" />
        </div>
        <div className="panel mt-2.5 h-12" />
      </div>
    </div>
  );
}

export function AppearanceTab() {
  const prefs = usePreferences();
  const set = prefs.set;

  return (
    <>
      <SettingSection>
        <div className="p-4">
          <ThemePreview />
        </div>
      </SettingSection>

      <SettingSection title="Thème">
        <SettingRow
          label="Mode"
          description="Le mode système suit le réglage clair/sombre de Windows."
          control={
            <ChoiceGroup
              value={prefs.themeMode}
              onChange={(themeMode) => set({ themeMode })}
              options={[
                { value: "dark", label: "Sombre", icon: Moon },
                { value: "light", label: "Clair", icon: Sun },
                { value: "system", label: "Système", icon: Monitor },
              ]}
            />
          }
        />
        <SettingRow
          label="Couleur d'accent"
          description="Boutons, dégradés, lueurs et éléments actifs."
          stacked
          control={
            <div className="flex flex-wrap items-center gap-2.5">
              {ACCENTS.map((accent) => (
                <button
                  key={accent}
                  title={ACCENT_LABELS[accent]}
                  aria-label={ACCENT_LABELS[accent]}
                  aria-pressed={prefs.accent === accent}
                  onClick={() => set({ accent })}
                  className={cn(
                    "flex size-9 items-center justify-center rounded-full ring-2 ring-transparent ring-offset-2 ring-offset-background transition-transform hover:scale-110",
                    prefs.accent === accent && "ring-foreground/60",
                  )}
                  style={{ backgroundColor: ACCENT_SWATCHES[accent] }}
                >
                  {prefs.accent === accent && (
                    <Check
                      className="size-4"
                      style={{ color: readableForeground(ACCENT_SWATCHES[accent]) }}
                      aria-hidden="true"
                    />
                  )}
                </button>
              ))}
              <label
                title="Couleur personnalisée"
                className={cn(
                  "relative flex size-9 cursor-pointer items-center justify-center overflow-hidden rounded-full ring-2 ring-transparent ring-offset-2 ring-offset-background transition-transform hover:scale-110",
                  prefs.accent === "custom" && "ring-foreground/60",
                )}
                style={{
                  background:
                    prefs.accent === "custom"
                      ? prefs.customAccent
                      : "conic-gradient(#ef4444, #f59e0b, #22c55e, #06b6d4, #6366f1, #d946ef, #ef4444)",
                }}
              >
                <Paintbrush
                  className="size-4 drop-shadow"
                  style={{ color: prefs.accent === "custom" ? readableForeground(prefs.customAccent) : "white" }}
                  aria-hidden="true"
                />
                <input
                  type="color"
                  aria-label="Couleur personnalisée"
                  value={isHexColor(prefs.customAccent) ? prefs.customAccent : "#22c55e"}
                  onChange={(e) => set({ accent: "custom", customAccent: e.target.value })}
                  className="absolute inset-0 cursor-pointer opacity-0"
                />
              </label>
            </div>
          }
        />
      </SettingSection>

      <SettingSection title="Ambiance">
        <SettingRow
          label="Fond d'écran"
          description="Un ciel de crépuscule, avec la couverture (ou l'icône) de l'instance sélectionnée par-dessus."
          control={
            <ChoiceGroup
              value={prefs.background}
              onChange={(background) => set({ background })}
              options={[
                { value: "image", label: "Image", icon: Image },
                { value: "gradient", label: "Dégradé", icon: Sparkle },
                { value: "solid", label: "Uni", icon: Square },
              ]}
            />
          }
        />
        {prefs.background === "image" && (
          <SettingRow
            label="Intensité du flou"
            control={
              <Slider
                className="w-48"
                min={0}
                max={100}
                step={5}
                value={[prefs.blurIntensity]}
                onValueChange={([blurIntensity]) => set({ blurIntensity })}
                aria-label="Intensité du flou"
              />
            }
          />
        )}
        <SettingRow
          label="Animations"
          description="« Réduites » garde les fondus mais coupe les mouvements."
          control={
            <ChoiceGroup
              value={prefs.animations}
              onChange={(animations) => set({ animations })}
              options={[
                { value: "full", label: "Complètes" },
                { value: "reduced", label: "Réduites" },
                { value: "none", label: "Aucune" },
              ]}
            />
          }
        />
      </SettingSection>

      <SettingSection title="Disposition">
        <SettingRow
          label="Taille de l'interface"
          description="Agrandit ou réduit tout le texte et les éléments."
          control={
            <ChoiceGroup
              value={String(prefs.uiScale)}
              onChange={(v) => set({ uiScale: Number(v) })}
              options={UI_SCALES.map((s) => ({ value: String(s), label: `${s} %` }))}
            />
          }
        />
      </SettingSection>

      <div className="flex justify-end">
        <Button variant="ghost" size="sm" onClick={prefs.resetAppearance}>
          Réinitialiser l'apparence
        </Button>
      </div>
    </>
  );
}
