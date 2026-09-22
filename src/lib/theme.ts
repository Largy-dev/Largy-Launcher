export const ACCENTS = ["green", "blue", "violet", "red", "orange", "cyan", "pink", "black", "white"] as const;

export type PresetAccent = (typeof ACCENTS)[number];
export type Accent = PresetAccent | "custom";

export const ACCENT_LABELS: Record<PresetAccent, string> = {
  green: "Vert",
  blue: "Bleu",
  violet: "Violet",
  red: "Rouge",
  orange: "Orange",
  cyan: "Cyan",
  pink: "Rose",
  black: "Graphite",
  white: "Argent",
};

/** Swatch color shown in the picker itself — independent of light/dark mode. */
export const ACCENT_SWATCHES: Record<PresetAccent, string> = {
  green: "#22c55e",
  blue: "#3b82f6",
  violet: "#8b5cf6",
  red: "#ef4444",
  orange: "#f97316",
  cyan: "#06b6d4",
  pink: "#ec4899",
  black: "#3f3f46",
  white: "#e4e4e7",
};

export type ThemeMode = "dark" | "light" | "system";
export type AnimationLevel = "full" | "reduced" | "none";

export interface ThemeSettings {
  themeMode: ThemeMode;
  accent: Accent;
  customAccent: string;
  uiScale: number;
  animations: AnimationLevel;
}

export const DEFAULT_ACCENT: PresetAccent = "green";

export function isAccent(value: unknown): value is Accent {
  return value === "custom" || (typeof value === "string" && (ACCENTS as readonly string[]).includes(value));
}

const HEX_RE = /^#([0-9a-f]{6})$/i;

export function isHexColor(value: string): boolean {
  return HEX_RE.test(value);
}

/** Black or white, whichever reads better on top of `hex` (WCAG relative luminance). */
export function readableForeground(hex: string): string {
  const match = HEX_RE.exec(hex);
  if (!match) return "#ffffff";
  const n = parseInt(match[1], 16);
  const channel = (c: number) => {
    const v = c / 255;
    return v <= 0.03928 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4;
  };
  const luminance = 0.2126 * channel((n >> 16) & 255) + 0.7152 * channel((n >> 8) & 255) + 0.0722 * channel(n & 255);
  return luminance > 0.4 ? "#0b0f19" : "#ffffff";
}

export function prefersDark(): boolean {
  return typeof window.matchMedia === "function" && window.matchMedia("(prefers-color-scheme: dark)").matches;
}

export function resolveDark(mode: ThemeMode): boolean {
  return mode === "dark" || (mode === "system" && prefersDark());
}

/** Pushes the visual preferences onto <html>: dark class, accent, custom color, scale, motion. */
export function applyTheme(settings: ThemeSettings): void {
  const root = document.documentElement;
  root.classList.toggle("dark", resolveDark(settings.themeMode));

  const accent = settings.accent === "custom" && !isHexColor(settings.customAccent) ? DEFAULT_ACCENT : settings.accent;
  root.setAttribute("data-accent", accent);
  if (accent === "custom") {
    root.style.setProperty("--accent-custom", settings.customAccent);
    root.style.setProperty("--accent-custom-fg", readableForeground(settings.customAccent));
  } else {
    root.style.removeProperty("--accent-custom");
    root.style.removeProperty("--accent-custom-fg");
  }

  root.style.fontSize = settings.uiScale === 100 ? "" : `${settings.uiScale}%`;
  root.setAttribute("data-motion", settings.animations);
}

const LEGACY_ACCENT_KEY = "largy-accent";

/** Accent saved by versions before the preferences store existed, if any. */
export function loadLegacyAccent(): Accent | null {
  try {
    const stored = localStorage.getItem(LEGACY_ACCENT_KEY);
    return isAccent(stored) ? stored : null;
  } catch {
    return null;
  }
}
