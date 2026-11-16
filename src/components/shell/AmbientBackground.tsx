import { convertFileSrc } from "@tauri-apps/api/core";
import { AnimatePresence, motion } from "motion/react";

import { useSelectedInstance } from "@/hooks/useInstances";
import { usePreferences } from "@/store/preferencesStore";

/** A few hundred pixels of faint stars, tiled over the top of the sky. */
const STARS = [
  "radial-gradient(1px 1px at 20px 30px, #fff 50%, transparent)",
  "radial-gradient(1px 1px at 120px 80px, #fff 50%, transparent)",
  "radial-gradient(1.5px 1.5px at 200px 20px, #fff 50%, transparent)",
  "radial-gradient(1px 1px at 60px 150px, #fff 50%, transparent)",
  "radial-gradient(1px 1px at 250px 170px, #fff 50%, transparent)",
  "radial-gradient(1.5px 1.5px at 160px 220px, #fff 50%, transparent)",
  "radial-gradient(1px 1px at 290px 110px, #fff 50%, transparent)",
].join(",");

/**
 * Full-window backdrop: a dusk sky (stars on top, a warm glow on the horizon
 * behind the content) with the selected instance's cover — or its icon,
 * heavily blurred — laid over it. "Dégradé" keeps only the sky, "Uni" none.
 */
export function AmbientBackground() {
  const background = usePreferences((s) => s.background);
  const blurIntensity = usePreferences((s) => s.blurIntensity);
  const selected = useSelectedInstance();

  if (background === "solid") return null;

  const cover = selected?.cover_path ? convertFileSrc(selected.cover_path) : null;
  const image = background === "image" ? (cover ?? selected?.icon_url ?? null) : null;
  const sharp = !!cover;

  return (
    <div aria-hidden="true" className="pointer-events-none fixed inset-0 -z-10 overflow-hidden bg-background">
      <div
        className="absolute inset-0"
        style={{
          backgroundImage: [
            "radial-gradient(70rem 26rem at 62% 108%, color-mix(in oklab, var(--sky-glow) 42%, transparent), transparent 70%)",
            "radial-gradient(45rem 30rem at 85% -5%, color-mix(in oklab, var(--accent-base) 26%, transparent), transparent 70%)",
            "linear-gradient(to bottom, var(--sky-top) 0%, var(--background) 55%, color-mix(in oklab, var(--sky-glow) 12%, var(--background)) 100%)",
          ].join(","),
        }}
      />
      <div
        className="absolute inset-0 opacity-0 dark:opacity-40"
        style={{
          backgroundImage: STARS,
          backgroundSize: "320px 260px",
          maskImage: "linear-gradient(to bottom, black, transparent 60%)",
        }}
      />
      <AnimatePresence>
        {image && (
          <motion.img
            key={image}
            src={image}
            alt=""
            initial={{ opacity: 0 }}
            animate={{ opacity: sharp ? 0.5 : 0.3 }}
            exit={{ opacity: 0 }}
            transition={{ duration: 0.8 }}
            className={
              sharp
                ? "absolute inset-x-0 top-0 h-[75%] w-full object-cover"
                : "absolute inset-[-10%] size-[120%] object-cover"
            }
            style={{
              filter: sharp
                ? `blur(${Math.round(blurIntensity * 0.12)}px) saturate(1.15)`
                : `blur(${24 + blurIntensity * 0.8}px) saturate(1.4)`,
              maskImage: sharp ? "linear-gradient(to bottom, black 20%, transparent)" : undefined,
            }}
          />
        )}
      </AnimatePresence>
      <div
        className="absolute inset-0"
        style={{
          backgroundImage:
            "linear-gradient(to right, color-mix(in oklab, var(--background) 55%, transparent), transparent 45%)",
        }}
      />
    </div>
  );
}
