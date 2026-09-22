import type { LoaderKind } from "@/services/tauri";

import { formatGb } from "./format";

export interface RamAdviceInput {
  loader: LoaderKind;
  /** Enabled mods, or null when unknown (not counted yet / no instance). */
  modCount: number | null;
  minecraftVersion: string;
  isModpack: boolean;
  systemTotalMb: number | null;
}

export interface RamAdvice {
  recommendedMb: number;
  /** Below this, lag spikes and OutOfMemory crashes become likely. */
  minOkMb: number;
  /** Above this, more RAM mostly just means longer GC pauses. */
  comfortableMaxMb: number;
  /** Most the game can take without starving Windows, or null if unknown. */
  maxSafeMb: number | null;
  reasons: string[];
}

export type RamStatus = "low" | "ok" | "high" | "danger";

/** Tailwind background class for each status's dot/zone. */
export const RAM_STATUS_CLASS: Record<RamStatus, string> = {
  ok: "bg-success",
  low: "bg-destructive",
  high: "bg-warning",
  danger: "bg-destructive",
};

/** RAM kept aside for Windows and background apps. */
const OS_RESERVE_MB = 3072;
/** Past this, Java's garbage collector pauses get noticeably longer. */
export const GC_WARNING_MB = 16384;

const LOADER_NAMES: Record<LoaderKind, string> = {
  vanilla: "Vanilla",
  forge: "Forge",
  neoforge: "NeoForge",
  fabric: "Fabric",
  quilt: "Quilt",
};

/** Minor version of a release id ("1.20.1" → 20). Snapshots and unknown ids count as recent. */
export function minecraftMinor(version: string): number {
  const match = /^1\.(\d+)/.exec(version);
  return match ? Number(match[1]) : 21;
}

const roundDown = (mb: number) => Math.floor(mb / 512) * 512;

function modsLabel(count: number | null, estimated: boolean): string {
  if (count === null) return "";
  return estimated ? ` (~${count} mods estimés)` : ` avec ${count} mod${count > 1 ? "s" : ""}`;
}

export function adviseRam(input: RamAdviceInput): RamAdvice {
  const minor = minecraftMinor(input.minecraftVersion);
  const reasons: string[] = [];
  const estimated = input.modCount === null && input.isModpack;
  const mods = input.modCount ?? (input.isModpack ? 120 : 20);

  let recommended: number;
  let minOk: number;

  if (input.loader === "vanilla") {
    recommended = minor <= 12 ? 2048 : 3072;
    minOk = minor <= 12 ? 1024 : 2048;
    reasons.push(`Minecraft ${input.minecraftVersion} vanilla : ${formatGb(recommended)} suffisent largement.`);
  } else {
    const heavy = input.loader === "forge" || input.loader === "neoforge";
    if (heavy) {
      recommended = mods < 50 ? 4608 : mods <= 150 ? 6144 : mods <= 250 ? 8192 : 10240;
      if (minor >= 18) {
        recommended += 1024;
        reasons.push("Les versions 1.18+ génèrent des mondes plus hauts et consomment plus.");
      }
    } else {
      recommended = mods < 50 ? 4096 : mods <= 150 ? 5120 : 6144;
    }
    minOk = Math.max(2048, recommended - (heavy ? 2048 : 1024));
    reasons.unshift(
      `${LOADER_NAMES[input.loader]}${modsLabel(input.modCount ?? (estimated ? mods : null), estimated)} : ` +
        `${formatGb(recommended)} conseillés.`,
    );
  }

  let comfortableMax = recommended + Math.max(2048, Math.round(recommended / 2));
  let maxSafe: number | null = null;

  if (input.systemTotalMb) {
    maxSafe = roundDown(Math.min(input.systemTotalMb - OS_RESERVE_MB, input.systemTotalMb * 0.6));
    maxSafe = Math.max(maxSafe, 1024);
    if (recommended > maxSafe) {
      recommended = maxSafe;
      reasons.push(
        `Ton PC a ${formatGb(input.systemTotalMb)} de RAM : conseil réduit à ${formatGb(recommended)} pour laisser respirer Windows.`,
      );
    }
    minOk = Math.min(minOk, maxSafe);
    comfortableMax = Math.min(comfortableMax, maxSafe);
  }

  return { recommendedMb: recommended, minOkMb: minOk, comfortableMaxMb: comfortableMax, maxSafeMb: maxSafe, reasons };
}

export function ramStatus(valueMb: number, advice: RamAdvice): { status: RamStatus; message: string } {
  if (advice.maxSafeMb !== null && valueMb > advice.maxSafeMb) {
    return { status: "danger", message: "Plus que ce que ton PC peut donner sans ralentir Windows." };
  }
  if (valueMb < advice.minOkMb) {
    return { status: "low", message: "Trop peu : risque de lags et de crashs « OutOfMemory »." };
  }
  if (valueMb > advice.comfortableMaxMb || valueMb > GC_WARNING_MB) {
    return {
      status: "high",
      message: "Beaucoup : au-delà, Java fait des pauses plus longues pour libérer la mémoire, sans gain de FPS.",
    };
  }
  return { status: "ok", message: "Bonne allocation pour cette instance." };
}
