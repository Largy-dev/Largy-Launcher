import {
  Check,
  Image,
  LayoutGrid,
  LayoutList,
  Monitor,
  Moon,
  Paintbrush,
  Rows3,
  Square,
  Sun,
  Sparkle,
} from "lucide-react";

import { ChoiceGroup, SettingRow, SettingSection } from "@/components/settings/SettingsKit";
import { Button } from "@/components/ui/button";
import { Slider } from "@/components/ui/slider";
import { ACCENTS, ACCENT_LABELS, ACCENT_SWATCHES, isHexColor, readableForeground } from "@/lib/theme";
import { cn } from "@/lib/utils";
import { usePreferences } from "@/store/preferencesStore";

const UI_SCALES = [90, 100, 110, 120];

/** Mini preview of the current theme: sidebar, hero and a card, in the live tokens. */
function ThemePreview() {
  return (
    <div className="relative h-36 overflow-hidden rounded-xl border border-border bg-background" aria-hidden="true">
      <div
        className="absolute inset-0"
        style={{
          backgroundImage:
            "radial-gradient(20rem 10rem at 90% 0%, color-mix(in oklab, var(--accent-base) 30%, transparent), transparent 70%)",
        }}
      />
      <div className="glass-strong absolute inset-y-0 left-0 w-16 space-y-1.5 border-y-0 border-l-0 p-2">
        <div className="bg-gradient-brand size-5 rounded-md" />
        <div className="h-2 rounded bg-accent" />
        <div className="h-2 w-3/4 rounded bg-muted" />
        <div className="h-2 w-2/3 rounded bg-muted" />
      </div>
      <div className="absolute inset-y-0 right-0 left-16 space-y-2 p-3">
        <div className="glass flex items-center gap-2 rounded-lg p-2">
          <div className="size-7 rounded-md bg-loader-neoforge/60" />
          <div className="flex-1 space-y-1">
            <div className="h-2 w-1/2 rounded bg-foreground/70" />
            <div className="h-1.5 w-1/3 rounded bg-muted-foreground/50" />
          </div>
          <div className="bg-gradient-brand shadow-glow h-5 w-12 rounded-md" />
        </div>
        <div className="grid grid-cols-3 gap-2">
          {["--loader-fabric", "--loader-forge", "--loader-vanilla"].map((v) => (
            <div key={v} className="glass h-12 overflow-hidden rounded-lg">
              <div className="h-4" style={{ backgroundColor: `color-mix(in oklab, var(${v}) 50%, transparent)` }} />
            </div>
          ))}
        </div>
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
          description="L'image de l'instance mise en avant, floutée derrière l'interface."
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
          label="Affichage des instances"
          control={
            <ChoiceGroup
              value={prefs.cardDensity}
              onChange={(cardDensity) => set({ cardDensity })}
              options={[
                { value: "grid", label: "Grille", icon: LayoutGrid },
                { value: "compact", label: "Compacte", icon: Rows3 },
                { value: "list", label: "Liste", icon: LayoutList },
              ]}
            />
          }
        />
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
