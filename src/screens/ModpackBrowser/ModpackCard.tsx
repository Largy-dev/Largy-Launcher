import { motion } from "motion/react";
import { Clock, Download, PackageSearch } from "lucide-react";

import { LOADER_META } from "@/components/instance/LoaderBadge";
import { formatCount, formatRelative } from "@/lib/format";
import { listItem } from "@/lib/motion";
import type { ModpackSummary } from "@/services/tauri";

/** A modpack in the catalogue: what it is, what it runs on, how alive it is. */
export function ModpackCard({ pack, index, onSelect }: { pack: ModpackSummary; index: number; onSelect: () => void }) {
  const version = pack.game_versions[0];
  return (
    <motion.button
      variants={listItem}
      custom={index}
      whileHover={{ y: -3 }}
      onClick={onSelect}
      aria-label={`${pack.name}${pack.author ? `, par ${pack.author}` : ""}`}
      className="glass group relative isolate flex transform-gpu flex-col gap-3 overflow-hidden rounded-2xl p-4 text-left transition-shadow will-change-transform hover:shadow-xl focus-visible:ring-2 focus-visible:ring-primary focus-visible:outline-none"
    >
      {pack.icon_url && (
        <img
          src={pack.icon_url}
          alt=""
          aria-hidden="true"
          className="pointer-events-none absolute -top-10 -right-10 -z-10 size-40 rounded-full object-cover opacity-20 blur-2xl transition-opacity group-hover:opacity-35"
        />
      )}
      <div className="flex items-start gap-3">
        {pack.icon_url ? (
          <img
            src={pack.icon_url}
            alt=""
            loading="lazy"
            className="size-14 shrink-0 rounded-xl object-cover shadow-md"
          />
        ) : (
          <div className="flex size-14 shrink-0 items-center justify-center rounded-xl bg-muted">
            <PackageSearch className="size-6 text-muted-foreground" aria-hidden="true" />
          </div>
        )}
        <div className="min-w-0 flex-1">
          <h3 className="line-clamp-2 leading-snug font-bold">{pack.name}</h3>
          {pack.author && <p className="truncate text-xs text-muted-foreground">par {pack.author}</p>}
        </div>
      </div>
      <p className="line-clamp-2 min-h-[2lh] text-xs text-muted-foreground">{pack.summary}</p>
      <div className="mt-auto flex flex-wrap items-center gap-1.5 text-[0.7rem]">
        {version && (
          <span className="rounded-md bg-muted px-1.5 py-0.5 font-semibold text-foreground/80 tabular-nums">
            {version}
          </span>
        )}
        {pack.loaders.slice(0, 2).map((loader) => (
          <span
            key={loader}
            className="rounded-md px-1.5 py-0.5 font-semibold"
            style={{
              color: `color-mix(in oklab, ${LOADER_META[loader].color} 80%, var(--foreground))`,
              backgroundColor: `color-mix(in oklab, ${LOADER_META[loader].color} 14%, transparent)`,
            }}
          >
            {LOADER_META[loader].label}
          </span>
        ))}
        <span className="ml-auto flex items-center gap-2.5 text-muted-foreground">
          {pack.downloads ? (
            <span className="flex items-center gap-1 tabular-nums" title="Téléchargements">
              <Download className="size-3" aria-hidden="true" />
              {formatCount(pack.downloads)}
            </span>
          ) : null}
          {pack.updated_at ? (
            <span className="flex items-center gap-1" title="Dernière mise à jour">
              <Clock className="size-3" aria-hidden="true" />
              {formatRelative(pack.updated_at)}
            </span>
          ) : null}
        </span>
      </div>
    </motion.button>
  );
}
