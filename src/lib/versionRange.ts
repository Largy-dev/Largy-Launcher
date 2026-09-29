/**
 * Just enough version-range matching to tell whether a mod's declared
 * incompatibility applies to the version actually installed: Maven ranges
 * (Forge / NeoForge `versionRange`) and npm-style predicates (Fabric /
 * Quilt `breaks`). Anything unparsable answers "no" — a missed warning is
 * better than a false alarm.
 */

function segments(version: string): string[] {
  return version
    .trim()
    .replace(/^v/i, "")
    .split(/[.+\-_]/)
    .filter((s) => s !== "");
}

/** Negative, zero or positive like `Array.sort`. Numeric segments compare as numbers. */
export function compareVersions(a: string, b: string): number {
  const sa = segments(a);
  const sb = segments(b);
  for (let i = 0; i < Math.max(sa.length, sb.length); i++) {
    const x = sa[i] ?? "0";
    const y = sb[i] ?? "0";
    const nx = /^\d+$/.test(x) ? Number(x) : NaN;
    const ny = /^\d+$/.test(y) ? Number(y) : NaN;
    const diff = !isNaN(nx) && !isNaN(ny) ? nx - ny : x.localeCompare(y);
    if (diff !== 0) return diff;
  }
  return 0;
}

function inMavenRange(version: string, range: string): boolean | null {
  const parts = range.match(/[[(][^\])]*[\])]/g);
  if (!parts) {
    // A bare version is a "soft" requirement: that version or newer.
    const bare = range.trim();
    return /^[\w.+-]+$/.test(bare) ? compareVersions(version, bare) >= 0 : null;
  }
  return parts.some((part) => {
    const open = part[0];
    const close = part[part.length - 1];
    const inner = part.slice(1, -1);
    if (!inner.includes(",")) return compareVersions(version, inner) === 0;
    const [low, high] = inner.split(",").map((s) => s.trim());
    if (low && (open === "[" ? compareVersions(version, low) < 0 : compareVersions(version, low) <= 0)) return false;
    if (high && (close === "]" ? compareVersions(version, high) > 0 : compareVersions(version, high) >= 0))
      return false;
    return true;
  });
}

function matchesComparator(version: string, comparator: string): boolean | null {
  const c = comparator.trim();
  if (!c || c === "*" || c.toLowerCase() === "x") return true;
  const m = c.match(/^(>=|<=|>|<|=|\^|~)?\s*([\w.+*-]+)$/);
  if (!m) return null;
  const [, op = "", raw] = m;
  const wildcard = raw.match(/^(.*?)(?:\.[xX*])+$/);
  const target = wildcard ? wildcard[1] : raw;
  const cmp = compareVersions(version, target);
  const parts = segments(target);
  const bump = (index: number) =>
    parts
      .slice(0, index + 1)
      .map((p, i) => (i === index ? String(Number(p) + 1) : p))
      .join(".");
  switch (op) {
    case ">=":
      return cmp >= 0;
    case ">":
      return cmp > 0;
    case "<=":
      return cmp <= 0;
    case "<":
      return cmp < 0;
    case "^": {
      const major = parts.findIndex((p) => p !== "0");
      return cmp >= 0 && compareVersions(version, bump(Math.max(0, major))) < 0;
    }
    case "~":
      return cmp >= 0 && compareVersions(version, bump(Math.min(1, parts.length - 1))) < 0;
    default:
      if (wildcard) return cmp >= 0 && compareVersions(version, bump(parts.length - 1)) < 0;
      return cmp === 0;
  }
}

function matchesNpm(version: string, predicates: string): boolean | null {
  const results = predicates.split("||").map((alt) => {
    const comparators = alt.trim().split(/\s+/).filter(Boolean);
    if (comparators.length === 0) return true;
    const each = comparators.map((c) => matchesComparator(version, c));
    return each.includes(null) ? null : each.every(Boolean);
  });
  if (results.includes(true)) return true;
  return results.includes(null) ? null : false;
}

/**
 * Whether `version` falls in `range`. A `null` range means every version.
 * False when the version is unknown and the range is bounded, or when the
 * range can't be parsed.
 */
export function satisfies(version: string | null, range: string | null, maven: boolean): boolean {
  if (range === null) return true;
  if (!version) return false;
  const result = maven ? inMavenRange(version, range) : matchesNpm(version, range);
  return result === true;
}
