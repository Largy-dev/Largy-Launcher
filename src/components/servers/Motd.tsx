import { parseMotdLines } from "@/lib/motd";
import { cn } from "@/lib/utils";

/** A server description with its in-game colours, one line per row. */
export function Motd({ text, className }: { text: string; className?: string }) {
  return (
    <div className={cn("font-mono text-xs leading-snug text-[#aaaaaa]", className)}>
      {parseMotdLines(text).map((line, i) => (
        <p key={i} className="truncate">
          {line.map((s, j) => (
            <span
              key={j}
              style={s.color ? { color: s.color } : undefined}
              className={cn(
                s.bold && "font-bold",
                s.italic && "italic",
                s.underline && "underline",
                s.strike && "line-through",
              )}
            >
              {s.text}
            </span>
          ))}
        </p>
      ))}
    </div>
  );
}
