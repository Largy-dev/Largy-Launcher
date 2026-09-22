import { useQuery } from "@tanstack/react-query";
import { AnimatePresence, motion } from "motion/react";

import { instancesApi, type Instance } from "@/services/tauri";
import { useAppStore } from "@/store/appStore";
import { usePreferences } from "@/store/preferencesStore";

/** The instance the launcher is "about": the one being looked at, else the last played. */
export function useFeaturedInstance(): Instance | undefined {
  const activeId = useAppStore((s) => s.activeInstanceId);
  const { data: instances } = useQuery({ queryKey: ["instances"], queryFn: instancesApi.list });
  if (!instances || instances.length === 0) return undefined;
  const active = activeId ? instances.find((i) => i.id === activeId) : undefined;
  if (active) return active;
  return [...instances].sort((a, b) => (b.last_played_at ?? b.created_at) - (a.last_played_at ?? a.created_at))[0];
}

/**
 * Full-window backdrop behind the app: the featured instance's art, heavily
 * blurred, under an accent-tinted gradient — or just the gradient / a plain
 * surface depending on the user's preference.
 */
export function AmbientBackground() {
  const background = usePreferences((s) => s.background);
  const blurIntensity = usePreferences((s) => s.blurIntensity);
  const featured = useFeaturedInstance();
  const image = background === "image" ? featured?.icon_url : null;

  if (background === "solid") return null;

  return (
    <div aria-hidden="true" className="pointer-events-none fixed inset-0 -z-10 overflow-hidden">
      <AnimatePresence>
        {image && (
          <motion.img
            key={image}
            src={image}
            alt=""
            initial={{ opacity: 0 }}
            animate={{ opacity: 0.35 }}
            exit={{ opacity: 0 }}
            transition={{ duration: 0.8 }}
            className="absolute inset-[-10%] size-[120%] object-cover"
            style={{ filter: `blur(${24 + blurIntensity * 0.8}px) saturate(1.4)` }}
          />
        )}
      </AnimatePresence>
      <div
        className="absolute inset-0"
        style={{
          backgroundImage: [
            "radial-gradient(60rem 40rem at 85% -10%, color-mix(in oklab, var(--accent-base) 22%, transparent), transparent 70%)",
            "radial-gradient(50rem 35rem at -10% 110%, color-mix(in oklab, #6d5dfc 14%, transparent), transparent 70%)",
            "linear-gradient(to bottom, color-mix(in oklab, var(--background) 55%, transparent), var(--background) 85%)",
          ].join(","),
        }}
      />
    </div>
  );
}
