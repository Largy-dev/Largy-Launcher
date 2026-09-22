import { useEffect, useState } from "react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { createHashRouter, NavLink, Outlet, RouterProvider } from "react-router";
import { Blocks, ChevronDown, LayoutGrid, LogIn, LogOut, Settings } from "lucide-react";
import { toast, Toaster } from "sonner";

import { cn } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { ThemeSwitcher } from "@/components/ThemeSwitcher";
import { RecentInstances } from "@/components/RecentInstances";
import { InstallWarningsDialog } from "@/components/InstallWarningsDialog";
import { InstanceListScreen } from "@/screens/InstanceList/InstanceListScreen";
import { ModpackBrowserScreen } from "@/screens/ModpackBrowser/ModpackBrowserScreen";
import { GlobalSettingsScreen } from "@/screens/GlobalSettings/GlobalSettingsScreen";
import { InstanceSettingsScreen } from "@/screens/InstanceSettings/InstanceSettingsScreen";
import { InstanceModsScreen } from "@/screens/InstanceMods/InstanceModsScreen";
import { LaunchProgressScreen } from "@/screens/LaunchProgress/LaunchProgressScreen";
import { LoginDialog } from "@/screens/Login/LoginScreen";
import { auth, errorMessage, onDownloadProgress, onInstanceExit, onInstanceLog } from "@/services/tauri";
import { checkForAppUpdate, installAppUpdate } from "@/lib/updater";
import { useAppStore } from "@/store/appStore";

const queryClient = new QueryClient();

const NAV_ITEMS = [
  { to: "/", label: "Instances", icon: LayoutGrid },
  { to: "/modpacks", label: "Modpacks", icon: Blocks },
  { to: "/settings", label: "Paramètres", icon: Settings },
];

function AccountArea() {
  const account = useAppStore((s) => s.account);
  const setAccount = useAppStore((s) => s.setAccount);
  const [loginOpen, setLoginOpen] = useState(false);

  async function logout() {
    await auth.logout();
    setAccount(null);
  }

  if (!account) {
    return (
      <>
        <Button onClick={() => setLoginOpen(true)} className="w-full justify-start gap-2">
          <LogIn className="size-4" aria-hidden="true" />
          Se connecter
        </Button>
        <LoginDialog open={loginOpen} onOpenChange={setLoginOpen} />
      </>
    );
  }

  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <button className="flex w-full items-center gap-2 rounded-md px-1 py-1 text-left hover:bg-sidebar-accent/60">
          <img src={`https://mc-heads.net/avatar/${account.profile.id}/32`} alt="" className="size-5 rounded-sm" />
          <p className="min-w-0 flex-1 truncate text-xs font-medium text-sidebar-foreground">{account.profile.name}</p>
          <ChevronDown className="size-3.5 text-sidebar-foreground/40" aria-hidden="true" />
        </button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="start" side="top">
        <DropdownMenuItem onClick={logout} className="gap-2 text-destructive">
          <LogOut className="size-4" aria-hidden="true" />
          Se déconnecter
        </DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}

function AppShell() {
  const setAccount = useAppStore((s) => s.setAccount);
  const setDownloadProgress = useAppStore((s) => s.setDownloadProgress);
  const appendLog = useAppStore((s) => s.appendLog);
  const setRunning = useAppStore((s) => s.setRunning);
  const setCrashAnalysis = useAppStore((s) => s.setCrashAnalysis);

  useEffect(() => {
    auth
      .trySilentLogin()
      .then(setAccount)
      .catch(() => setAccount(null));
  }, [setAccount]);

  useEffect(() => {
    checkForAppUpdate()
      .then((update) => {
        if (!update) return;
        toast.info(`Nouvelle version disponible : v${update.currentVersion} → v${update.version}`, {
          duration: Infinity,
          action: {
            label: "Mettre à jour",
            onClick: () => {
              const id = toast.loading("Téléchargement de la mise à jour…");
              installAppUpdate(update, (percent) => toast.loading(`Téléchargement… ${percent}%`, { id })).catch((e) =>
                toast.error(errorMessage(e), { id }),
              );
            },
          },
        });
      })
      .catch(() => {
        // Pas de connexion, GitHub indisponible, etc. — on ne bloque jamais le démarrage pour ça.
      });
  }, []);

  useEffect(() => {
    const unlisten = [
      onDownloadProgress((p) => setDownloadProgress(p)),
      onInstanceLog((l) => appendLog(l.instance_id, l.line)),
      onInstanceExit((e) => {
        setRunning(e.instance_id, false);
        setCrashAnalysis(e.instance_id, e.crash_analysis);
      }),
    ];
    return () => {
      unlisten.forEach((p) => p.then((fn) => fn()));
    };
  }, [appendLog, setCrashAnalysis, setDownloadProgress, setRunning]);

  return (
    <div className="flex h-screen w-screen overflow-hidden bg-background text-foreground">
      <aside className="flex w-56 shrink-0 flex-col border-r border-sidebar-border bg-sidebar text-sidebar-foreground">
        <div className="flex items-center gap-2 px-4 py-4">
          <div className="flex size-8 shrink-0 items-center justify-center rounded-md bg-primary text-sm font-bold text-primary-foreground">
            L
          </div>
          <h1 className="text-sm font-semibold leading-tight">Largy Launcher</h1>
        </div>

        <nav aria-label="Navigation principale" className="flex flex-col gap-0.5 px-2">
          {NAV_ITEMS.map(({ to, label, icon: Icon }) => (
            <NavLink
              key={to}
              to={to}
              end={to === "/"}
              className={({ isActive }) =>
                cn(
                  "flex items-center gap-2 rounded-md border-l-2 px-2.5 py-2 text-sm transition-colors",
                  isActive
                    ? "border-l-brand bg-sidebar-accent font-medium text-sidebar-accent-foreground"
                    : "border-l-transparent text-sidebar-foreground/65 hover:bg-sidebar-accent/60 hover:text-sidebar-foreground",
                )
              }
            >
              <Icon className="size-4" aria-hidden="true" />
              {label}
            </NavLink>
          ))}
          <ThemeSwitcher />
        </nav>

        <div className="flex-1 overflow-y-auto">
          <RecentInstances />
        </div>

        <div className="border-t border-sidebar-border px-3 py-3">
          <AccountArea />
        </div>
      </aside>

      <main className="flex flex-1 flex-col overflow-y-auto p-6">
        <Outlet />
      </main>
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
      { path: "settings", element: <GlobalSettingsScreen /> },
      { path: "instances/:id", element: <InstanceSettingsScreen /> },
      { path: "instances/:id/mods", element: <InstanceModsScreen /> },
      { path: "instances/:id/launch", element: <LaunchProgressScreen /> },
    ],
  },
]);

function App() {
  return (
    <QueryClientProvider client={queryClient}>
      <RouterProvider router={router} />
      <Toaster richColors position="bottom-right" />
      <InstallWarningsDialog />
    </QueryClientProvider>
  );
}

export default App;
