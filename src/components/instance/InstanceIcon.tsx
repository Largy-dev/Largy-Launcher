import { MinecraftGrassIcon } from "@/components/MinecraftGrassIcon";
import { cn } from "@/lib/utils";
import type { Instance } from "@/services/tauri";

import { LOADER_META } from "./LoaderBadge";

/** Modpack icon, grass block for vanilla, or the loader's icon on a tinted tile. */
export function InstanceIcon({ instance, className }: { instance: Instance; className?: string }) {
  if (instance.icon_url) {
    return <img src={instance.icon_url} alt="" className={cn("shrink-0 rounded-lg object-cover", className)} />;
  }
  if (instance.loader === "vanilla") {
    return <MinecraftGrassIcon className={cn("shrink-0 rounded-lg", className)} />;
  }
  const meta = LOADER_META[instance.loader];
  const Icon = meta.icon;
  return (
    <div
      className={cn("flex shrink-0 items-center justify-center rounded-lg", className)}
      style={{
        backgroundImage: `linear-gradient(135deg, color-mix(in oklab, ${meta.color} 45%, transparent), color-mix(in oklab, ${meta.color} 15%, transparent))`,
      }}
    >
      <Icon className="size-1/2 text-foreground/85" aria-hidden="true" />
    </div>
  );
}
