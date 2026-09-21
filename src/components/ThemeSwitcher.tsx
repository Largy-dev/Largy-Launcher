import { useState } from "react";
import { Check, Palette } from "lucide-react";

import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { ACCENTS, ACCENT_LABELS, ACCENT_SWATCHES, applyAccent, loadAccent, type Accent } from "@/lib/theme";

export function ThemeSwitcher() {
  const [accent, setAccent] = useState<Accent>(() => loadAccent());

  function choose(next: Accent) {
    setAccent(next);
    applyAccent(next);
  }

  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <Button
          variant="ghost"
          size="icon-sm"
          title="Changer le thème"
          className="text-sidebar-foreground/60 hover:text-sidebar-foreground"
        >
          <Palette className="size-4" aria-hidden="true" />
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="start" side="top" className="w-40">
        {ACCENTS.map((option) => (
          <DropdownMenuItem key={option} onClick={() => choose(option)} className="gap-2">
            <span
              className="size-3.5 shrink-0 rounded-full border border-black/10"
              style={{ backgroundColor: ACCENT_SWATCHES[option] }}
              aria-hidden="true"
            />
            <span className="flex-1">{ACCENT_LABELS[option]}</span>
            {accent === option && <Check className="size-3.5 text-muted-foreground" aria-hidden="true" />}
          </DropdownMenuItem>
        ))}
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
