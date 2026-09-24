import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { AnimatePresence, MotionConfig, motion } from "motion/react";
import { createHashRouter, RouterProvider, useLocation, useOutlet } from "react-router";

import { Toaster } from "@/components/ui/sonner";
import { InstallWarningsDialog } from "@/components/InstallWarningsDialog";
import { ActivityBar } from "@/components/shell/ActivityBar";
import { AmbientBackground } from "@/components/shell/AmbientBackground";
import { CloseDialog } from "@/components/shell/CloseDialog";
import { Sidebar } from "@/components/shell/Sidebar";
import { useAppEvents } from "@/hooks/useAppEvents";
import { useLaunchRequests } from "@/hooks/useLaunchRequests";
import { pageTransition, reducedMotionFor } from "@/lib/motion";
import { resolveDark } from "@/lib/theme";
import { InstanceListScreen } from "@/screens/InstanceList/InstanceListScreen";
import { ModpackBrowserScreen } from "@/screens/ModpackBrowser/ModpackBrowserScreen";
import { GlobalSettingsScreen } from "@/screens/GlobalSettings/GlobalSettingsScreen";
import { InstanceSettingsScreen } from "@/screens/InstanceSettings/InstanceSettingsScreen";
import { InstanceModsScreen } from "@/screens/InstanceMods/InstanceModsScreen";
import { LaunchProgressScreen } from "@/screens/LaunchProgress/LaunchProgressScreen";
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
        key={location.pathname}
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

function AppShell() {
  useAppEvents();
  useLaunchRequests();

  return (
    <div className="flex h-screen w-screen overflow-hidden text-foreground">
      <AmbientBackground />
      <Sidebar />
      <main className="relative flex flex-1 flex-col overflow-y-auto p-6">
        <ActivityBar />
        <AnimatedOutlet />
      </main>
      <CloseDialog />
    </div>
  );
}

const router = createHashRouter([
  {
    path: "/",
    element: <AppShell />,
    children: [
      { index: true, element: <InstanceListScreen /> },
      { path: "modpacks", element: <ModpackBrowserScreen /> },
      { path: "skins", element: <SkinsScreen /> },
      { path: "settings", element: <GlobalSettingsScreen /> },
      { path: "instances/:id", element: <InstanceSettingsScreen /> },
      { path: "instances/:id/mods", element: <InstanceModsScreen /> },
      { path: "instances/:id/launch", element: <LaunchProgressScreen /> },
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
