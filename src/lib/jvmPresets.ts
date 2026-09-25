export interface JvmPreset {
  id: string;
  label: string;
  description: string;
  args: string[];
  /** Lowest Minecraft minor version whose bundled Java supports these flags. */
  minMinecraftMinor?: number;
  /** Presets sharing a group are mutually exclusive (the JVM accepts one GC only). */
  group?: string;
}

export const JVM_PRESETS: JvmPreset[] = [
  {
    id: "aikar",
    label: "G1GC optimisé",
    group: "gc",
    description: "Les flags d'Aikar : moins de micro-saccades sur les gros modpacks. Le choix sûr.",
    args: [
      "-XX:+UseG1GC",
      "-XX:+ParallelRefProcEnabled",
      "-XX:MaxGCPauseMillis=200",
      "-XX:+UnlockExperimentalVMOptions",
      "-XX:+DisableExplicitGC",
      "-XX:+AlwaysPreTouch",
      "-XX:G1NewSizePercent=30",
      "-XX:G1MaxNewSizePercent=40",
      "-XX:G1HeapRegionSize=8M",
      "-XX:G1ReservePercent=20",
      "-XX:G1HeapWastePercent=5",
      "-XX:G1MixedGCCountTarget=4",
      "-XX:InitiatingHeapOccupancyPercent=15",
      "-XX:G1MixedGCLiveThresholdPercent=90",
      "-XX:G1RSetUpdatingPauseTimePercent=5",
      "-XX:SurvivorRatio=32",
      "-XX:+PerfDisableSharedMem",
      "-XX:MaxTenuringThreshold=1",
    ],
  },
  {
    id: "zgc",
    label: "ZGC générationnel",
    group: "gc",
    description: "Pauses quasi nulles avec beaucoup de RAM. Nécessite Java 21 (Minecraft 1.21+).",
    args: ["-XX:+UseZGC", "-XX:+ZGenerational"],
    minMinecraftMinor: 21,
  },
  {
    id: "utf8",
    label: "Encodage UTF-8",
    description: "Corrige les accents cassés dans certains mods et noms de mondes.",
    args: ["-Dfile.encoding=UTF-8"],
  },
];

export function isPresetActive(args: string[], preset: JvmPreset): boolean {
  return preset.args.every((a) => args.includes(a));
}

/**
 * Adds the preset's flags (keeping the user's others, minus those of presets
 * in the same group), or removes exactly them.
 */
export function togglePreset(args: string[], preset: JvmPreset): string[] {
  if (isPresetActive(args, preset)) return args.filter((a) => !preset.args.includes(a));
  const rivals = new Set(
    JVM_PRESETS.filter((p) => preset.group && p.group === preset.group && p.id !== preset.id).flatMap((p) => p.args),
  );
  const kept = args.filter((a) => !rivals.has(a) || preset.args.includes(a));
  return [...kept, ...preset.args.filter((a) => !kept.includes(a))];
}
