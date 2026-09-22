import { useState, type MouseEvent } from "react";

import { formatGb } from "@/lib/format";

const W = 300;
const H = 56;
const PAD = 4;
/** Seconds between two samples (matches the stats polling interval). */
const SAMPLE_SECONDS = 2;

interface MemorySparklineProps {
  samples: number[];
  allocatedMb: number;
}

/**
 * RAM used by the game over the last ~90 s. Single series, so no legend —
 * the caption names it; the dashed line is the allocation ceiling.
 * Hovering shows the value at that point.
 */
export function MemorySparkline({ samples, allocatedMb }: MemorySparklineProps) {
  const [hover, setHover] = useState<number | null>(null);
  const max = Math.max(allocatedMb, ...samples) * 1.05;
  const x = (i: number) => (samples.length <= 1 ? W : PAD + (i / (samples.length - 1)) * (W - PAD * 2));
  const y = (mb: number) => H - PAD - (mb / max) * (H - PAD * 2);
  const line = samples.map((mb, i) => `${i === 0 ? "M" : "L"}${x(i).toFixed(1)},${y(mb).toFixed(1)}`).join(" ");
  const area = samples.length > 1 ? `${line} L${x(samples.length - 1)},${H - PAD} L${x(0)},${H - PAD} Z` : "";
  const shown = hover ?? samples.length - 1;

  function onMove(e: MouseEvent<SVGSVGElement>) {
    if (samples.length < 2) return;
    const rect = e.currentTarget.getBoundingClientRect();
    const ratio = (e.clientX - rect.left) / rect.width;
    setHover(Math.max(0, Math.min(samples.length - 1, Math.round(ratio * (samples.length - 1)))));
  }

  return (
    <figure className="min-w-0">
      <figcaption className="mb-1 flex justify-between text-[0.7rem] text-muted-foreground">
        <span>Mémoire du jeu</span>
        {samples.length > 0 && (
          <span className="font-medium text-foreground tabular-nums">
            {formatGb(samples[shown])}
            {hover !== null && (
              <span className="font-normal text-muted-foreground">
                {" "}
                · il y a {(samples.length - 1 - hover) * SAMPLE_SECONDS} s
              </span>
            )}
          </span>
        )}
      </figcaption>
      <svg
        viewBox={`0 0 ${W} ${H}`}
        preserveAspectRatio="none"
        className="h-14 w-full cursor-crosshair overflow-visible"
        role="img"
        aria-label={`Mémoire utilisée : ${samples.length ? formatGb(samples[samples.length - 1]) : "inconnue"} sur ${formatGb(allocatedMb)} allouées`}
        onMouseMove={onMove}
        onMouseLeave={() => setHover(null)}
      >
        <line
          x1={0}
          x2={W}
          y1={y(allocatedMb)}
          y2={y(allocatedMb)}
          stroke="var(--muted-foreground)"
          strokeOpacity={0.5}
          strokeDasharray="4 4"
          vectorEffect="non-scaling-stroke"
        />
        {area && <path d={area} fill="var(--accent-base)" fillOpacity={0.14} />}
        {samples.length > 1 && (
          <path
            d={line}
            fill="none"
            stroke="var(--accent-base)"
            strokeWidth={2}
            strokeLinejoin="round"
            strokeLinecap="round"
            vectorEffect="non-scaling-stroke"
          />
        )}
        {hover !== null && (
          <line
            x1={x(hover)}
            x2={x(hover)}
            y1={0}
            y2={H}
            stroke="var(--foreground)"
            strokeOpacity={0.3}
            vectorEffect="non-scaling-stroke"
          />
        )}
      </svg>
    </figure>
  );
}
