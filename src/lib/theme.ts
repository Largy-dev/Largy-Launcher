export const ACCENTS = ["green", "black", "white", "violet", "red", "blue"] as const;

export type Accent = (typeof ACCENTS)[number];

export const ACCENT_LABELS: Record<Accent, string> = {
  green: "Vert",
  black: "Noir",
  white: "Blanc",
  violet: "Violet",
  red: "Rouge",
  blue: "Bleu",
};

/** Swatch color shown in the picker itself — independent of light/dark mode. */
export const ACCENT_SWATCHES: Record<Accent, string> = {
  green: "#1b7a3e",
  black: "#27272a",
  white: "#e4e4e7",
  violet: "#7c3aed",
  red: "#dc2626",
  blue: "#2563eb",
};

const STORAGE_KEY = "largy-accent";
const DEFAULT_ACCENT: Accent = "green";

function isAccent(value: string | null): value is Accent {
  return !!value && (ACCENTS as readonly string[]).includes(value);
}

export function loadAccent(): Accent {
  try {
    const stored = localStorage.getItem(STORAGE_KEY);
    return isAccent(stored) ? stored : DEFAULT_ACCENT;
  } catch {
    return DEFAULT_ACCENT;
  }
}

export function applyAccent(accent: Accent): void {
  document.documentElement.setAttribute("data-accent", accent);
  try {
    localStorage.setItem(STORAGE_KEY, accent);
  } catch {
    // Best-effort only — a private window or blocked storage just won't persist the choice.
  }
}
