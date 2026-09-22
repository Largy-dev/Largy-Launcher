import { Lightbulb, Sparkles } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Slider } from "@/components/ui/slider";
import { formatGb } from "@/lib/format";
import { ramStatus, RAM_STATUS_CLASS, type RamAdvice, type RamStatus } from "@/lib/ramAdvice";
import { cn } from "@/lib/utils";

interface MemorySliderProps {
  valueMb: number;
  onChangeMb: (mb: number) => void;
  maxMb: number;
  minMb?: number;
  stepMb?: number;
  /** When given, the track shows the advice zones, a "conseillé" marker and a verdict. */
  advice?: RamAdvice | null;
  className?: string;
}

const STATUS_TEXT: Record<RamStatus, string> = {
  ok: "text-success",
  low: "text-destructive",
  high: "text-warning",
  danger: "text-destructive",
};

const STATUS_LABEL: Record<RamStatus, string> = {
  ok: "Idéal",
  low: "Insuffisant",
  high: "Élevé",
  danger: "Trop pour ton PC",
};

/** RAM allocation slider with colored advice zones (too little / good / a lot / beyond the PC). */
export function MemorySlider({
  valueMb,
  onChangeMb,
  maxMb,
  minMb = 1024,
  stepMb = 512,
  advice,
  className,
}: MemorySliderProps) {
  const max = Math.max(maxMb, minMb + stepMb);
  const clamped = Math.min(Math.max(valueMb, minMb), max);
  const pct = (mb: number) => `${((Math.min(Math.max(mb, minMb), max) - minMb) / (max - minMb)) * 100}%`;
  const verdict = advice ? ramStatus(clamped, advice) : null;

  const zones = advice
    ? [
        { from: minMb, to: advice.minOkMb, status: "low" as const },
        { from: advice.minOkMb, to: advice.comfortableMaxMb, status: "ok" as const },
        { from: advice.comfortableMaxMb, to: advice.maxSafeMb ?? max, status: "high" as const },
        { from: advice.maxSafeMb ?? max, to: max, status: "danger" as const },
      ].filter((z) => z.to > z.from)
    : [];

  return (
    <div className={cn("w-full space-y-2", className)}>
      <div className="flex items-baseline justify-between">
        <span className="text-2xl font-black tabular-nums">{formatGb(clamped)}</span>
        {verdict && (
          <span className={cn("flex items-center gap-1.5 text-xs font-semibold", STATUS_TEXT[verdict.status])}>
            <span className={cn("size-2 rounded-full", RAM_STATUS_CLASS[verdict.status])} aria-hidden="true" />
            {STATUS_LABEL[verdict.status]}
          </span>
        )}
      </div>

      <div className="relative pt-4">
        {advice && (
          <>
            <div className="absolute inset-x-0 top-0 flex h-1.5 overflow-hidden rounded-full" aria-hidden="true">
              {zones.map((z) => (
                <div
                  key={z.status}
                  className={cn(RAM_STATUS_CLASS[z.status], "opacity-70")}
                  style={{ width: `calc(${pct(z.to)} - ${pct(z.from)})` }}
                />
              ))}
            </div>
            <div
              className="absolute -top-0.5 h-2.5 w-0.5 -translate-x-1/2 rounded-full bg-foreground"
              style={{ left: pct(advice.recommendedMb) }}
              title={`Conseillé : ${formatGb(advice.recommendedMb)}`}
              aria-hidden="true"
            />
          </>
        )}
        <Slider
          min={minMb}
          max={max}
          step={stepMb}
          value={[clamped]}
          onValueChange={([v]) => onChangeMb(v)}
          aria-label="Mémoire maximale allouée"
        />
      </div>

      <div className="flex justify-between text-[0.7rem] text-muted-foreground tabular-nums">
        <span>{formatGb(minMb)}</span>
        {advice && <span className="font-medium text-foreground">Conseillé : {formatGb(advice.recommendedMb)}</span>}
        <span>{formatGb(max)}</span>
      </div>

      {advice && verdict && (
        <div className="flex items-start gap-3 rounded-xl bg-muted/60 p-3 text-xs">
          <Lightbulb className="mt-0.5 size-4 shrink-0 text-warning" aria-hidden="true" />
          <div className="flex-1 space-y-1">
            <p className={cn("font-medium", STATUS_TEXT[verdict.status])}>{verdict.message}</p>
            {advice.reasons.map((reason) => (
              <p key={reason} className="text-muted-foreground">
                {reason}
              </p>
            ))}
          </div>
          {clamped !== advice.recommendedMb && (
            <Button size="xs" variant="outline" className="gap-1" onClick={() => onChangeMb(advice.recommendedMb)}>
              <Sparkles aria-hidden="true" />
              Appliquer
            </Button>
          )}
        </div>
      )}
    </div>
  );
}
