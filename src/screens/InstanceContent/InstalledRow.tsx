import { memo } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import {
  AlertTriangle,
  ExternalLink,
  FolderOpen,
  Image as ImageIcon,
  MoreHorizontal,
  Puzzle,
  Sparkles,
  Trash2,
  type LucideIcon,
} from "lucide-react";

import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Switch } from "@/components/ui/switch";
import { formatBytes } from "@/lib/format";
import { displayName } from "@/lib/installedContent";
import { cn } from "@/lib/utils";
import type { InstalledItem, RemoteProvider } from "@/services/content";
import type { ContentKind } from "@/services/tauri";

export const ROW_HEIGHT = 68;

const KIND_ICON: Record<ContentKind, LucideIcon> = { mod: Puzzle, resource_pack: ImageIcon, shader: Sparkles };

export const PROVIDER_META: Record<RemoteProvider, { label: string; color: string }> = {
  modrinth: { label: "Modrinth", color: "#1bd96a" },
  curseforge: { label: "CurseForge", color: "#f16436" },
};

function ItemIcon({ item, kind }: { item: InstalledItem; kind: ContentKind }) {
  const src = item.icon_path ? convertFileSrc(item.icon_path) : item.remote?.icon_url;
  if (src) {
    return (
      <img
        src={src}
        alt=""
        loading="lazy"
        decoding="async"
        className={cn(
          "size-10 shrink-0 rounded-lg bg-muted object-cover",
          item.icon_path && "[image-rendering:pixelated]",
        )}
      />
    );
  }
  const Icon = KIND_ICON[kind];
  return (
    <div className="flex size-10 shrink-0 items-center justify-center rounded-lg bg-primary/10 text-primary">
      <Icon className="size-4.5" aria-hidden="true" />
    </div>
  );
}

interface InstalledRowProps {
  item: InstalledItem;
  kind: ContentKind;
  selected: boolean;
  selecting: boolean;
  missing: string[] | undefined;
  conflicts: string[] | undefined;
  requiredBy: string[] | undefined;
  busy: boolean;
  onSelect: (item: InstalledItem, selected: boolean, shift: boolean) => void;
  onToggle: (item: InstalledItem, enabled: boolean) => void;
  onDelete: (item: InstalledItem) => void;
  onReveal: () => void;
  onFindDependency: (id: string) => void;
}

/** One installed mod / pack: identity, source, dependency problems, and its actions. */
export const InstalledRow = memo(function InstalledRow({
  item,
  kind,
  selected,
  selecting,
  missing,
  conflicts,
  requiredBy,
  busy,
  onSelect,
  onToggle,
  onDelete,
  onReveal,
  onFindDependency,
}: InstalledRowProps) {
  const name = displayName(item);
  const source = item.remote ? PROVIDER_META[item.remote.provider] : null;
  const subtitle = [
    item.authors.length > 0 ? `par ${item.authors.slice(0, 2).join(", ")}` : null,
    item.remote?.description || item.description,
  ]
    .filter(Boolean)
    .join(" · ");

  return (
    <div
      className={cn(
        "group flex h-full items-center gap-3 px-3 transition-colors",
        selected ? "bg-primary/8" : "hover:bg-foreground/[0.03]",
      )}
      onClick={(e) => {
        if (selecting && !(e.target as HTMLElement).closest("button,[role=switch],[role=menuitem],a")) {
          onSelect(item, !selected, e.shiftKey);
        }
      }}
    >
      <Checkbox
        checked={selected}
        aria-label={`Sélectionner ${name}`}
        onClick={(e) => {
          e.stopPropagation();
          onSelect(item, !selected, e.shiftKey);
        }}
        className={cn(
          "transition-opacity",
          selecting || selected ? "opacity-100" : "opacity-0 group-hover:opacity-100 focus-visible:opacity-100",
        )}
      />
      <div className={cn("flex min-w-0 flex-1 items-center gap-3", !item.enabled && "opacity-50 grayscale")}>
        <ItemIcon item={item} kind={kind} />
        <div className="min-w-0 flex-1">
          <div className="flex min-w-0 items-center gap-2">
            <p className="truncate text-sm font-semibold" title={item.file_name}>
              {name}
            </p>
            {item.version && (
              <span className="shrink-0 rounded-md bg-muted px-1.5 py-px text-[0.68rem] font-medium text-muted-foreground tabular-nums">
                {item.version.length > 18 ? `${item.version.slice(0, 18)}…` : item.version}
              </span>
            )}
            {source && item.remote && (
              <button
                type="button"
                onClick={() => openUrl(item.remote!.url)}
                title={`Ouvrir sur ${source.label}`}
                className="inline-flex shrink-0 items-center gap-1 rounded-md px-1 py-px text-[0.68rem] font-medium text-muted-foreground transition-colors hover:bg-muted hover:text-foreground"
              >
                <span className="size-1.5 rounded-full" style={{ backgroundColor: source.color }} aria-hidden="true" />
                {source.label}
              </button>
            )}
          </div>
          <p className="truncate text-xs text-muted-foreground">
            {subtitle || item.file_name}
            <span className="opacity-70"> · {formatBytes(item.size)}</span>
          </p>
        </div>
      </div>

      {missing && missing.length > 0 && (
        <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <button
              type="button"
              className="inline-flex shrink-0 items-center gap-1 rounded-full bg-destructive/12 px-2 py-0.5 text-[0.7rem] font-semibold text-destructive transition-colors hover:bg-destructive/20"
            >
              <AlertTriangle className="size-3" aria-hidden="true" />
              {missing.length === 1 ? `Requiert ${missing[0]}` : `${missing.length} dépendances absentes`}
            </button>
          </DropdownMenuTrigger>
          <DropdownMenuContent align="end" className="w-64">
            <p className="px-2 py-1.5 text-xs text-muted-foreground">
              Ces mods sont requis mais absents ou désactivés. Cherche-les dans le catalogue :
            </p>
            {missing.map((id) => (
              <DropdownMenuItem key={id} className="gap-2" onClick={() => onFindDependency(id)}>
                <Puzzle className="size-3.5" aria-hidden="true" />
                {id}
              </DropdownMenuItem>
            ))}
          </DropdownMenuContent>
        </DropdownMenu>
      )}

      {conflicts && conflicts.length > 0 && (
        <span
          className="inline-flex max-w-52 shrink-0 items-center gap-1 truncate rounded-full bg-amber-500/15 px-2 py-0.5 text-[0.7rem] font-semibold text-amber-600 dark:text-amber-400"
          title={`Déclaré incompatible avec ${conflicts.join(", ")} : désactive l'un des deux.`}
        >
          <AlertTriangle className="size-3 shrink-0" aria-hidden="true" />
          <span className="truncate">Incompatible avec {conflicts.join(", ")}</span>
        </span>
      )}

      {!item.is_dir && (
        <Switch
          checked={item.enabled}
          disabled={busy}
          aria-label={item.enabled ? `Désactiver ${name}` : `Activer ${name}`}
          title={item.enabled && requiredBy?.length ? `Requis par ${requiredBy.slice(0, 3).join(", ")}` : undefined}
          onCheckedChange={(enabled) => onToggle(item, enabled)}
        />
      )}
      <DropdownMenu>
        <DropdownMenuTrigger asChild>
          <Button variant="ghost" size="icon-sm" aria-label={`Actions pour ${name}`}>
            <MoreHorizontal aria-hidden="true" />
          </Button>
        </DropdownMenuTrigger>
        <DropdownMenuContent align="end" className="w-52">
          {item.remote && source && (
            <DropdownMenuItem className="gap-2" onClick={() => openUrl(item.remote!.url)}>
              <ExternalLink className="size-3.5" aria-hidden="true" />
              Page {source.label}
            </DropdownMenuItem>
          )}
          <DropdownMenuItem className="gap-2" onClick={onReveal}>
            <FolderOpen className="size-3.5" aria-hidden="true" />
            Afficher dans le dossier
          </DropdownMenuItem>
          <DropdownMenuSeparator />
          <DropdownMenuItem className="gap-2 text-destructive focus:text-destructive" onClick={() => onDelete(item)}>
            <Trash2 className="size-3.5" aria-hidden="true" />
            Supprimer
          </DropdownMenuItem>
        </DropdownMenuContent>
      </DropdownMenu>
    </div>
  );
});
