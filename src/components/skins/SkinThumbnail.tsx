import { useEffect, useState } from "react";

import { renderSkinFront } from "@/lib/skinPreview";
import { cn } from "@/lib/utils";
import type { SkinVariant } from "@/services/skins";

/** Flat front view of a skin, drawn once from its texture. */
export function SkinThumbnail({
  texture,
  variant,
  className,
}: {
  texture: string;
  variant: SkinVariant;
  className?: string;
}) {
  const [src, setSrc] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    renderSkinFront(texture, variant)
      .then((url) => !cancelled && setSrc(url))
      .catch(() => {});
    return () => {
      cancelled = true;
    };
  }, [texture, variant]);

  return src ? (
    <img src={src} alt="" className={cn("h-24 w-12 [image-rendering:pixelated]", className)} />
  ) : (
    <div className={cn("h-24 w-12", className)} />
  );
}
