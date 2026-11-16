import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { AnimatePresence, MotionConfig, motion } from "motion/react";
import {
  createHashRouter,
  Navigate,
  RouterProvider,
  useLocation,
  useOutlet,
  useParams,
  useSearchParams,
} from "react-router";

import { Toaster } from "@/components/ui/sonner";
import { InstallWarningsDialog } from "@/components/InstallWarningsDialog";
import { ActivityBar } from "@/components/shell/ActivityBar";
import { AmbientBackground } from "@/components/shell/AmbientBackground";
import { CloseDialog } from "@/components/shell/CloseDialog";
import { CommandPalette } from "@/components/shell/CommandPalette";
import { Sidebar } from "@/components/shell/Sidebar";
import { TopBar } from "@/components/shell/TopBar";
import { useState } from "react";

import { useAppEvents } from "@/hooks/useAppEvents";
import { useFileDrop } from "@/hooks/useFileDrop";
import { useImportInstance } from "@/hooks/useImportInstance";
import { useLaunchRequests } from "@/hooks/useLaunchRequests";
import { usePlaytimeReminder } from "@/hooks/usePlaytimeReminder";
import { ScrollContainerContext } from "@/hooks/useScrollContainer";
import { pageTransition, reducedMotionFor } from "@/lib/motion";
import { resolveDark } from "@/lib/theme";
import { InstanceScreen } from "@/screens/Instance/InstanceScreen";
import { ModpackBrowserScreen } from "@/screens/ModpackBrowser/ModpackBrowserScreen";
import { GlobalSettingsScreen } from "@/screens/GlobalSettings/GlobalSettingsScreen";
import { ServerBrowserScreen } from "@/screens/ServerBrowser/ServerBrowserScreen";
import { SkinsScreen } from "@/screens/Skins/SkinsScreen";
import { usePreferences } from "@/store/preferencesStore";

const queryClient = new QueryClient();

/** Keeps the previous page rendered while it animates out. */
function AnimatedOutlet() {
  const location = useLocation();
  const outlet = useOutlet();
  return (
    <AnimatePresence mode="wait" initial={false}>
      <motion.div
        // The home and every instance share one page: switching instance
        // swaps its content in place instead of replaying the page transition.
        key={location.pathname.startsWith("/instances/") ? "/" : location.pathname}
        variants={pageTransition}
        initial="initial"
        animate="enter"
        exit="exit"
        className="flex flex-1 flex-col"
      >
        {outlet}
      </motion.div>
    </AnimatePresence>
  );
}

/** Pages that used to be separate screens now live in the instance page's tabs. */
function LegacyInstanceRedirect({ tab }: { tab?: string }) {
  const { id } = useParams<{ id: string }>();
  const [params] = useSearchParams();
  const legacy = params.get("tab");
  const mapped =
    tab ?? (legacy === "resource_pack" || legacy === "shader" ? "packs" : legacy === "worlds" ? "worlds" : "mods");
  return <Navigate replace to={`/instances/${id}?tab=${mapped}`} />;
}

function AppShell() {
  useAppEvents();
  useLaunchRequests();
  usePlaytimeReminder();
  const [scrollEl, setScrollEl] = useState<HTMLElement | null>(null);
  const { importPaths } = useImportInstance();
  const dragging = useFileDrop(importPaths);

  return (
    <div className="flex h-screen w-screen overflow-hidden text-foreground">
      <AmbientBackground />
      <Sidebar />
      <div className="flex min-w-0 flex-1 flex-col">
        <TopBar />
        <main ref={setScrollEl} className="relative flex flex-1 flex-col overflow-y-auto px-6 pt-2 pb-6">
          <ScrollContainerContext.Provider value={scrollEl}>
            <ActivityBar />
            <AnimatedOutlet />
          </ScrollContainerContext.Provider>
        </main>
      </div>
      {dragging && (
        <div className="pointer-events-none fixed inset-4 z-50 flex items-center justify-center rounded-3xl border-2 border-dashed border-primary bg-background/80 backdrop-blur-sm">
          <p className="text-lg font-semibold text-primary">Dépose un .mrpack ou un .zip pour l'importer</p>
        </div>
      )}
      <CloseDialog />
      <CommandPalette />
    </div>
  );
}

const router = createHashRouter([
  {
    path: "/",
    element: <AppShell />,
    children: [
      { index: true, element: <InstanceScreen /> },
      { path: "modpacks", element: <ModpackBrowserScreen /> },
      { path: "servers", element: <ServerBrowserScreen /> },
      { path: "skins", element: <SkinsScreen /> },
      { path: "settings", element: <GlobalSettingsScreen /> },
      { path: "instances/:id", element: <InstanceScreen /> },
      { path: "instances/:id/content", element: <LegacyInstanceRedirect /> },
      { path: "instances/:id/mods", element: <LegacyInstanceRedirect tab="mods" /> },
      { path: "instances/:id/launch", element: <LegacyInstanceRedirect tab="console" /> },
    ],
  },
]);

function App() {
  const animations = usePreferences((s) => s.animations);
  const themeMode = usePreferences((s) => s.themeMode);

  return (
    <QueryClientProvider client={queryClient}>
      <MotionConfig reducedMotion={reducedMotionFor(animations)}>
        <RouterProvider router={router} />
        <Toaster richColors closeButton position="bottom-right" theme={resolveDark(themeMode) ? "dark" : "light"} />
        <InstallWarningsDialog />
      </MotionConfig>
    </QueryClientProvider>
  );
}

export default App;
