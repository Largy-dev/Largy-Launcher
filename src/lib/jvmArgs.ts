/** Splits a space-separated JVM args string into an array, dropping empty tokens. */
export function parseJvmArgs(value: string): string[] {
  return value.trim().split(/\s+/).filter(Boolean);
}

const isGcSelector = (arg: string) => /^-XX:\+Use\w*GC$/.test(arg);

/**
 * Explains, in the settings, what the launcher will fix at launch time (see
 * `launch/jvm_args.rs`): combinations that would stop Java from starting,
 * which exits before Minecraft writes any log. `minecraftMinor` enables the
 * Java-version checks; omit it for the global defaults.
 */
export function jvmArgIssues(args: string[], minecraftMinor?: number): string[] {
  const issues: string[] = [];
  const gcs = [...new Set(args.filter(isGcSelector))];
  if (gcs.length > 1) {
    issues.push(
      `Plusieurs ramasse-miettes activés (${gcs.join(", ")}) : Java n'en accepte qu'un, seul le dernier (${gcs[gcs.length - 1]}) sera gardé.`,
    );
  }
  if (
    minecraftMinor !== undefined &&
    minecraftMinor < 21 &&
    args.some((a) => a.includes("ZGenerational") || a === "-XX:+UseZGC")
  ) {
    issues.push("ZGC demande Java 21 (Minecraft 1.21+) : il sera ignoré pour cette version.");
  }
  let valueExpected = false;
  for (const arg of args) {
    if (!valueExpected && !arg.startsWith("-")) {
      issues.push(`« ${arg} » n'est pas un argument JVM (ils commencent par « - ») : il sera ignoré.`);
    }
    valueExpected = !valueExpected && arg.startsWith("--") && !arg.includes("=");
  }
  if (args.some((a) => a.startsWith("-Xmx") || a.startsWith("-Xms"))) {
    issues.push("-Xmx / -Xms ici remplacent la RAM réglée dans l'onglet Mémoire.");
  }
  return issues;
}
