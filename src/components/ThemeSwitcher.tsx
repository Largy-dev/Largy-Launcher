import { useState } from "react";
import { Check, Palette } from "lucide-react";

import { cn } from "@/lib/utils";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { ACCENTS, ACCENT_LABELS, ACCENT_SWATCHES, applyAccent, loadAccent, type Accent } from "@/lib/theme";

export function ThemeSwitcher({ className }: { className?: string }) {
  const [accent, setAccent] = useState<Accent>(() => loadAccent());

  function choose(next: Accent) {
    setAccent(next);
    applyAccent(next);
  }

  return (
    <Popover>
      <PopoverTrigger asChild>
        <button
          className={cn(
            "flex items-center gap-2 rounded-md border-l-2 border-l-transparent px-2.5 py-2 text-sm text-sidebar-foreground/65 transition-colors hover:bg-sidebar-accent/60 hover:text-sidebar-foreground",
            className,
          )}
        >
          <Palette className="size-4" aria-hidden="true" />
          Thème
        </button>
      </PopoverTrigger>
      <PopoverContent side="right" align="end" className="w-auto">
        <p className="mb-2.5 text-xs font-medium text-muted-foreground">Couleur d'accent</p>
        <div className="flex flex-wrap gap-2.5">
          {ACCENTS.map((option) => (
            <button
              key={option}
              title={ACCENT_LABELS[option]}
              onClick={() => choose(option)}
              className="relative flex size-8 items-center justify-center rounded-full ring-1 ring-black/10 transition-transform hover:scale-110"
              style={{ backgroundColor: ACCENT_SWATCHES[option] }}
            >
              {accent === option && (
                <Check
                  className={cn("size-4", option === "white" ? "text-zinc-900" : "text-white")}
                  aria-hidden="true"
                />
              )}
            </button>
          ))}
        </div>
      </PopoverContent>
    </Popover>
  );
}
