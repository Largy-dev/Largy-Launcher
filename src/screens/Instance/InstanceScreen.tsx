import { useEffect, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { Navigate, useParams, useSearchParams } from "react-router";
import { AnimatePresence, motion } from "motion/react";

import { Skeleton } from "@/components/Skeleton";
import { useInstances, useSelectedInstance } from "@/hooks/useInstances";
import { spring } from "@/lib/motion";
import { cn } from "@/lib/utils";
import { ContentBrowserDialog } from "@/screens/InstanceContent/ContentBrowserDialog";
import { WorldsTab } from "@/screens/InstanceContent/WorldsTab";
import { ScreenshotsTab } from "@/screens/InstanceSettings/ScreenshotsTab";
import { ServersTab } from "@/screens/InstanceSettings/ServersTab";
import { installedApi } from "@/services/content";
import { serversApi } from "@/services/servers";
import type { ContentKind, Instance } from "@/services/tauri";
import { runtimeOf, useAppStore } from "@/store/appStore";

import { ConsoleTab } from "./ConsoleTab";
import { ContentTab } from "./ContentTab";
import { InstanceHeader } from "./InstanceHeader";
import { OverviewTab } from "./OverviewTab";
import { SettingsTab } from "./SettingsTab";
import { resolveTab, tabCount, tabsFor, type InstanceTab } from "./tabs";
import { Welcome } from "./Welcome";

/** Scrolls the tab strip (only it, never the page) so this tab sits in its middle. */
function centerInStrip(tab: HTMLElement | null) {
  const strip = tab?.parentElement;
  if (!tab || !strip || strip.scrollWidth <= strip.clientWidth) return;
  const offset = tab.getBoundingClientRect().left - strip.getBoundingClientRect().left;
  strip.scrollLeft += offset - (strip.clientWidth - tab.offsetWidth) / 2;
}

interface BrowseState {
  open: boolean;
  kind: ContentKind;
  query?: string;
  nonce: number;
}

function InstanceView({ instance }: { instance: Instance }) {
  const [params, setParams] = useSearchParams();
  const tab = resolveTab(instance, params.get("tab"));
  const runtime = useAppStore((s) => runtimeOf(s.runtime, instance.id));
  const crashed = useAppStore((s) => !!s.crashAnalysis[instance.id]);
  const [browse, setBrowse] = useState<BrowseState>({ open: false, kind: "mod", nonce: 0 });

  const { data: summary } = useQuery({
    queryKey: ["content-summary", instance.id],
    queryFn: () => installedApi.summary(instance.id),
  });
  const { data: servers } = useQuery({
    queryKey: ["instance-servers", instance.id],
    queryFn: () => serversApi.list(instance.id),
  });

  const go = (next: InstanceTab) => setParams(next === "overview" ? {} : { tab: next }, { replace: true });
  const openBrowser = (kind: ContentKind, query?: string) =>
    setBrowse((b) => ({ open: true, kind, query, nonce: b.nonce + 1 }));

  return (
    <div className="mx-auto flex w-full max-w-6xl flex-1 flex-col">
      <InstanceHeader
        instance={instance}
        onBrowseMods={() => {
          go("mods");
          openBrowser("mod");
        }}
      />

      <div className="flex flex-1 flex-col">
        <div
          role="tablist"
          aria-label="Sections de l'instance"
          className="tile flex w-fit max-w-full gap-0.5 overflow-x-auto rounded-b-none border-b-0 p-1.5 no-scrollbar"
        >
          {tabsFor(instance).map(({ id, label, icon: Icon }) => {
            const active = id === tab;
            const count = id === "servers" ? (servers?.length ?? null) : tabCount(id, summary);
            const live = id === "console" && (runtime.running || crashed);
            return (
              <button
                key={id}
                role="tab"
                aria-selected={active}
                // Narrow window: the strip scrolls, so keep the open tab in view.
                ref={active ? centerInStrip : undefined}
                onClick={() => go(id)}
                className={cn(
                  "relative flex shrink-0 items-center gap-1.5 rounded-lg px-3 py-1.5 text-sm transition-colors",
                  active ? "font-semibold text-foreground" : "text-foreground/60 hover:text-foreground",
                )}
              >
                {active && (
                  <motion.span
                    layoutId="instance-tab"
                    transition={spring}
                    className="absolute inset-0 rounded-lg bg-foreground/10 ring-1 ring-foreground/10"
                  />
                )}
                <Icon className="relative size-4" aria-hidden="true" />
                <span className="relative">{label}</span>
                {count !== null && count > 0 && (
                  <span className="relative text-xs font-normal text-muted-foreground tabular-nums">{count}</span>
                )}
                {live && (
                  <span
                    className={cn(
                      "relative size-1.5 rounded-full",
                      crashed && !runtime.running ? "bg-destructive" : "animate-pulse bg-success",
                    )}
                    aria-hidden="true"
                  />
                )}
              </button>
            );
          })}
        </div>

        <div data-panel className="panel flex-1 rounded-tl-none p-5">
          <AnimatePresence mode="wait" initial={false}>
            <motion.div
              key={tab}
              initial={{ opacity: 0, y: 6 }}
              animate={{ opacity: 1, y: 0 }}
              exit={{ opacity: 0, y: -4 }}
              transition={{ duration: 0.16 }}
            >
              {tab === "overview" && <OverviewTab instance={instance} onNavigate={go} />}
              {tab === "mods" && <ContentTab instance={instance} kinds={["mod"]} onBrowse={openBrowser} />}
              {tab === "packs" && (
                <ContentTab instance={instance} kinds={["resource_pack", "shader"]} onBrowse={openBrowser} />
              )}
              {tab === "worlds" && <WorldsTab instance={instance} />}
              {tab === "servers" && <ServersTab instance={instance} />}
              {tab === "screenshots" && <ScreenshotsTab instance={instance} />}
              {tab === "console" && <ConsoleTab instance={instance} />}
              {tab === "settings" && <SettingsTab instance={instance} />}
            </motion.div>
          </AnimatePresence>
        </div>
      </div>

      <ContentBrowserDialog
        key={browse.nonce}
        instance={instance}
        open={browse.open}
        onOpenChange={(o) => setBrowse((b) => ({ ...b, open: o }))}
        initialKind={browse.kind}
        initialQuery={browse.query}
      />
    </div>
  );
}

/**
 * The home page: the instance picked in the sidebar (or the last played),
 * with everything about it in tabs. Welcomes new players when there's none.
 */
export function InstanceScreen() {
  const { id } = useParams<{ id: string }>();
  const { data: instances, isLoading } = useInstances();
  const selected = useSelectedInstance();
  const setActiveInstanceId = useAppStore((s) => s.setActiveInstanceId);
  const requested = id ? instances?.find((i) => i.id === id) : undefined;
  const instance = id ? requested : selected;

  useEffect(() => {
    if (id && requested) setActiveInstanceId(id);
  }, [id, requested, setActiveInstanceId]);

  if (isLoading) {
    return (
      <div className="mx-auto w-full max-w-6xl space-y-4 pt-8">
        <Skeleton className="h-6 w-48 rounded-lg" />
        <Skeleton className="h-14 w-96 rounded-xl" />
        <Skeleton className="h-14 w-full max-w-2xl rounded-xl" />
        <Skeleton className="mt-6 h-80 rounded-2xl" />
      </div>
    );
  }
  if (!instances || instances.length === 0) return <Welcome />;
  // A deleted (or mistyped) instance: back to the home page.
  if (id && !requested) return <Navigate replace to="/" />;
  if (!instance) return null;
  return <InstanceView key={instance.id} instance={instance} />;
}
