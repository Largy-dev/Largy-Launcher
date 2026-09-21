import { Slider } from "@/components/ui/slider";
import { cn } from "@/lib/utils";

interface MemorySliderProps {
  valueMb: number;
  onChangeMb: (mb: number) => void;
  maxMb: number;
  minMb?: number;
  stepMb?: number;
  className?: string;
}

/** RAM allocation bar: min/max in Mo, current value highlighted in Go. */
export function MemorySlider({ valueMb, onChangeMb, maxMb, minMb = 512, stepMb = 256, className }: MemorySliderProps) {
  const max = Math.max(maxMb, minMb + stepMb);
  const clamped = Math.min(Math.max(valueMb, minMb), max);

  return (
    <div className={cn("w-64 space-y-1.5", className)}>
      <Slider min={minMb} max={max} step={stepMb} value={[clamped]} onValueChange={([v]) => onChangeMb(v)} />
      <div className="flex justify-between text-xs text-muted-foreground">
        <span>{minMb} Mo</span>
        <span className="font-medium text-foreground">{(clamped / 1024).toFixed(1)} Go</span>
        <span>{(max / 1024).toFixed(0)} Go</span>
      </div>
    </div>
  );
}
