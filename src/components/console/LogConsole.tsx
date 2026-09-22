import { useDeferredValue, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { AnimatePresence, motion } from "motion/react";
import { ArrowDown, Copy, FolderOpen, Search, WrapText } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { countByLevel, parseLogs, type LogLevel, type ParsedLogLine, type RawLogLine } from "@/lib/logParse";
import { notify } from "@/lib/notify";
import { cn } from "@/lib/utils";
import { usePreferences } from "@/store/preferencesStore";

const LEVEL_STYLE: Record<LogLevel, { label: string; text: string; chip: string }> = {
  debug: { label: "DEBUG", text: "text-muted-foreground/70", chip: "text-muted-foreground bg-muted" },
  info: { label: "INFO", text: "text-foreground/85", chip: "text-info bg-info/12" },
  warn: { label: "WARN", text: "text-warning", chip: "text-warning bg-warning/12" },
  error: { label: "ERREUR", text: "text-destructive", chip: "text-destructive bg-destructive/12" },
};

const FILTERABLE: LogLevel[] = ["info", "warn", "error"];

function Highlight({ text, needle }: { text: string; needle: string }) {
  if (!needle) return <>{text}</>;
  const lower = text.toLowerCase();
  const parts: ReactNode[] = [];
  let from = 0;
  let index = lower.indexOf(needle, from);
  while (index !== -1) {
    parts.push(text.slice(from, index));
    parts.push(
      <mark key={index} className="rounded-sm bg-warning/40 text-foreground">
        {text.slice(index, index + needle.length)}
      </mark>,
    );
    from = index + needle.length;
    index = lower.indexOf(needle, from);
  }
  parts.push(text.slice(from));
  return <>{parts}</>;
}

function LogRow({ line, needle }: { line: ParsedLogLine; needle: string }) {
  const style = LEVEL_STYLE[line.level];
  return (
    <div className={cn("flex gap-2 px-3 py-px hover:bg-foreground/5", line.level === "error" && "bg-destructive/5")}>
      {line.time ? (
        <span className="w-[8ch] shrink-0 text-muted-foreground/60 tabular-nums">{line.time.slice(-8)}</span>
      ) : (
        <span className="w-[8ch] shrink-0" />
      )}
      <span className={cn("min-w-0 flex-1", style.text, line.stack && "pl-4 opacity-80")}>
        {line.source && <span className="mr-1.5 text-muted-foreground/70">[{line.source}]</span>}
        <Highlight text={line.message} needle={needle} />
      </span>
    </div>
  );
}

interface LogConsoleProps {
  lines: RawLogLine[];
  onOpenFolder?: () => void;
  className?: string;
}

/** Colored, filterable, searchable game log with smart auto-scroll. */
export function LogConsole({ lines, onOpenFolder, className }: LogConsoleProps) {
  const autoScroll = usePreferences((s) => s.logs.autoScroll);
  const wrap = usePreferences((s) => s.logs.wrap);
  const setLogs = usePreferences((s) => s.setLogs);
  const [hidden, setHidden] = useState<Set<LogLevel>>(new Set());
  const [search, setSearch] = useState("");
  const [pinned, setPinned] = useState(true);
  const scrollRef = useRef<HTMLDivElement>(null);

  const deferredLines = useDeferredValue(lines);
  const parsed = useMemo(() => parseLogs(deferredLines), [deferredLines]);
  const counts = useMemo(() => countByLevel(parsed), [parsed]);
  const needle = search.trim().toLowerCase();
  const visible = useMemo(
    () =>
      parsed.filter(
        (l) => !hidden.has(l.level === "debug" ? "info" : l.level) && (!needle || l.raw.toLowerCase().includes(needle)),
      ),
    [parsed, hidden, needle],
  );

  const following = autoScroll && pinned;
  useEffect(() => {
    const node = scrollRef.current;
    if (following && node) node.scrollTop = node.scrollHeight;
  }, [visible, following]);

  function onScroll() {
    const node = scrollRef.current;
    if (!node) return;
    const atBottom = node.scrollHeight - node.scrollTop - node.clientHeight < 40;
    if (atBottom !== pinned) setPinned(atBottom);
  }

  function toggle(level: LogLevel) {
    setHidden((current) => {
      const next = new Set(current);
      if (next.has(level)) next.delete(level);
      else next.add(level);
      return next;
    });
  }

  async function copyAll() {
    try {
      await navigator.clipboard.writeText(visible.map((l) => l.raw).join("\n"));
      notify.success({ title: `${visible.length} lignes copiées`, history: false });
    } catch {
      notify.error({ title: "Impossible de copier dans le presse-papiers", history: false });
    }
  }

  return (
    <div className={cn("glass flex min-h-0 flex-col overflow-hidden rounded-2xl", className)}>
      <div className="flex flex-wrap items-center gap-2 border-b border-border/60 px-3 py-2">
        <div className="flex gap-1">
          {FILTERABLE.map((level) => {
            const style = LEVEL_STYLE[level];
            const count = level === "info" ? counts.info + counts.debug : counts[level];
            const off = hidden.has(level);
            return (
              <button
                key={level}
                onClick={() => toggle(level)}
                aria-pressed={!off}
                className={cn(
                  "flex items-center gap-1.5 rounded-md px-2 py-1 text-[0.68rem] font-bold transition-opacity",
                  style.chip,
                  off && "opacity-35",
                )}
              >
                {style.label}
                <span className="font-medium tabular-nums opacity-80">{count}</span>
              </button>
            );
          })}
        </div>
        <div className="relative ml-auto">
          <Search
            className="pointer-events-none absolute top-1/2 left-2 size-3.5 -translate-y-1/2 text-muted-foreground"
            aria-hidden="true"
          />
          <Input
            type="search"
            placeholder="Filtrer les logs…"
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            className="h-7 w-48 pl-7! text-xs"
          />
        </div>
        <Button
          variant="ghost"
          size="icon-sm"
          title={wrap ? "Ne pas couper les lignes" : "Couper les lignes longues"}
          aria-pressed={wrap}
          onClick={() => setLogs({ wrap: !wrap })}
          className={cn(wrap && "text-primary")}
        >
          <WrapText aria-hidden="true" />
        </Button>
        <Button variant="ghost" size="icon-sm" title="Copier les logs affichés" onClick={copyAll}>
          <Copy aria-hidden="true" />
        </Button>
        {onOpenFolder && (
          <Button variant="ghost" size="icon-sm" title="Ouvrir le dossier de l'instance" onClick={onOpenFolder}>
            <FolderOpen aria-hidden="true" />
          </Button>
        )}
      </div>

      <div className="relative min-h-0 flex-1">
        <div
          ref={scrollRef}
          onScroll={onScroll}
          className={cn(
            "absolute inset-0 overflow-auto py-2 font-mono text-[0.72rem] leading-relaxed",
            wrap ? "whitespace-pre-wrap break-all" : "whitespace-pre",
          )}
        >
          {visible.length === 0 ? (
            <p className="px-3 text-muted-foreground">
              {parsed.length === 0 ? "En attente des premiers logs…" : "Aucune ligne ne correspond aux filtres."}
            </p>
          ) : (
            visible.map((line, i) => <LogRow key={i} line={line} needle={needle} />)
          )}
        </div>

        <AnimatePresence>
          {!pinned && (
            <motion.button
              initial={{ opacity: 0, y: 8 }}
              animate={{ opacity: 1, y: 0 }}
              exit={{ opacity: 0, y: 8 }}
              onClick={() => {
                setPinned(true);
                const node = scrollRef.current;
                if (node) node.scrollTop = node.scrollHeight;
              }}
              className="bg-gradient-brand shadow-glow absolute right-4 bottom-4 flex items-center gap-1.5 rounded-full px-3 py-1.5 text-xs font-semibold text-primary-foreground"
            >
              <ArrowDown className="size-3.5" aria-hidden="true" />
              Derniers logs
            </motion.button>
          )}
        </AnimatePresence>
      </div>
    </div>
  );
}
