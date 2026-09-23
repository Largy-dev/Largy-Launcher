/** « 42 h 10 min », « 12 min », « 45 s » — compact French duration. */
export function formatDuration(totalSeconds: number): string {
  const seconds = Math.max(0, Math.floor(totalSeconds));
  const hours = Math.floor(seconds / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  if (hours > 0) return minutes > 0 ? `${hours} h ${minutes} min` : `${hours} h`;
  if (minutes > 0) return `${minutes} min`;
  return `${seconds} s`;
}

/** « 01:05:09 » — running-session clock. */
export function formatClock(totalSeconds: number): string {
  const seconds = Math.max(0, Math.floor(totalSeconds));
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${pad(Math.floor(seconds / 3600))}:${pad(Math.floor((seconds % 3600) / 60))}:${pad(seconds % 60)}`;
}

/** « à l'instant », « il y a 5 min », « hier », « il y a 3 j », then a date. `unixSeconds` like the backend. */
export function formatRelative(unixSeconds: number, now: number = Date.now()): string {
  const diff = Math.max(0, Math.floor(now / 1000 - unixSeconds));
  if (diff < 60) return "à l'instant";
  if (diff < 3600) return `il y a ${Math.floor(diff / 60)} min`;
  if (diff < 86400) return `il y a ${Math.floor(diff / 3600)} h`;
  const days = Math.floor(diff / 86400);
  if (days === 1) return "hier";
  if (days < 30) return `il y a ${days} j`;
  return new Date(unixSeconds * 1000).toLocaleDateString("fr-FR", { day: "numeric", month: "short", year: "numeric" });
}

const BYTE_UNITS = ["o", "Ko", "Mo", "Go", "To"];

/** « 1,5 Mo » — French decimal comma, one decimal above Ko. */
export function formatBytes(bytes: number): string {
  let value = Math.max(0, bytes);
  let unit = 0;
  while (value >= 1024 && unit < BYTE_UNITS.length - 1) {
    value /= 1024;
    unit++;
  }
  const digits = unit === 0 ? 0 : 1;
  return `${value.toFixed(digits).replace(".", ",")} ${BYTE_UNITS[unit]}`;
}

export function formatSpeed(bytesPerSecond: number): string {
  return `${formatBytes(bytesPerSecond)}/s`;
}

/** Megabytes shown as gigabytes: « 6 Go », « 5,5 Go ». */
export function formatGb(mb: number): string {
  const gb = mb / 1024;
  const rounded = Math.round(gb * 10) / 10;
  return `${Number.isInteger(rounded) ? rounded : rounded.toFixed(1).replace(".", ",")} Go`;
}

/** Remaining time for a transfer, or null when it can't be estimated yet. */
export function formatEta(bytesRemaining: number, bytesPerSecond: number): string | null {
  if (bytesPerSecond <= 0 || bytesRemaining <= 0) return null;
  const seconds = Math.ceil(bytesRemaining / bytesPerSecond);
  return seconds < 60 ? `${seconds} s` : formatDuration(seconds);
}

/** 950 → "950", 12 300 → "12,3 k", 4 500 000 → "4,5 M". */
export function formatCount(n: number): string {
  const fmt = (v: number) => v.toLocaleString("fr-FR", { maximumFractionDigits: 1 });
  if (n >= 1_000_000) return `${fmt(n / 1_000_000)} M`;
  if (n >= 1_000) return `${fmt(n / 1_000)} k`;
  return String(n);
}
