import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { NavLink, useNavigate } from "react-router";
import { motion } from "motion/react";
import {
  Blocks,
  ChevronUp,
  History,
  LayoutGrid,
  LogIn,
  LogOut,
  Palette,
  Server,
  Settings,
  Shirt,
  UserPlus,
  UserRound,
} from "lucide-react";

import { InstanceIcon } from "@/components/instance/InstanceIcon";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { useAccounts } from "@/hooks/useAccounts";
import { useSettings } from "@/hooks/useSettings";
import { useAppVersion } from "@/hooks/useAppVersion";
import { useNow } from "@/hooks/useInstanceInfo";
import { formatClock } from "@/lib/format";
import { spring } from "@/lib/motion";
import { cn } from "@/lib/utils";
import { LoginDialog } from "@/screens/Login/LoginScreen";
import { instancesApi, type Instance } from "@/services/tauri";
import { runtimeOf, useAppStore } from "@/store/appStore";

import { NotificationCenter } from "./NotificationCenter";

const NAV_ITEMS = [
  { to: "/", label: "Instances", icon: LayoutGrid },
  { to: "/modpacks", label: "Modpacks", icon: Blocks },
  { to: "/servers", label: "Serveurs", icon: Server },
  { to: "/skins", label: "Skins", icon: Shirt },
  { to: "/settings", label: "Paramètres", icon: Settings },
];

function SectionTitle({ icon: Icon, children }: { icon: typeof History; children: string }) {
  return (
    <p className="flex items-center gap-1.5 px-2.5 pt-4 pb-1 text-[0.68rem] font-semibold tracking-wider text-sidebar-foreground/40 uppercase">
      <Icon className="size-3" aria-hidden="true" />
      {children}
    </p>
  );
}

function InstanceShortcut({ instance }: { instance: Instance }) {
  const navigate = useNavigate();
  const runtime = useAppStore((s) => runtimeOf(s.runtime, instance.id));
  const setActiveInstanceId = useAppStore((s) => s.setActiveInstanceId);
  const now = useNow(1000, runtime.running && !!runtime.startedAt);

  return (
    <button
      onClick={() => {
        setActiveInstanceId(instance.id);
        navigate(runtime.running ? `/instances/${instance.id}/launch` : `/instances/${instance.id}`);
      }}
      className="group flex items-center gap-2.5 rounded-lg px-2.5 py-1.5 text-left text-sm text-sidebar-foreground/70 transition-colors hover:bg-sidebar-accent hover:text-sidebar-foreground"
    >
      <InstanceIcon instance={instance} className="size-6 rounded-md transition-transform group-hover:scale-110" />
      <span className="min-w-0 flex-1 truncate">{instance.name}</span>
      {runtime.running && (
        <span className="flex items-center gap-1.5 text-[0.68rem] text-primary tabular-nums">
          {runtime.startedAt && formatClock((now - runtime.startedAt) / 1000)}
          <span className="size-2 animate-pulse-glow rounded-full bg-primary" aria-label="En cours" />
        </span>
      )}
    </button>
  );
}

function AccountCard() {
  const account = useAppStore((s) => s.account);
  const { data: settings } = useSettings();
  const { others, switchTo, forget } = useAccounts();
  const navigate = useNavigate();
  const [loginOpen, setLoginOpen] = useState(false);

  if (!account) {
    if (settings?.offline_mode) {
      return (
        <button
          onClick={() => navigate("/settings?tab=account")}
          className="flex w-full items-center gap-2.5 rounded-lg p-1.5 text-left transition-colors hover:bg-sidebar-accent"
        >
          <div className="flex size-8 items-center justify-center rounded-md bg-muted">
            <UserRound className="size-4 text-muted-foreground" aria-hidden="true" />
          </div>
          <div className="min-w-0 flex-1">
            <p className="truncate text-sm font-medium">{settings.offline_username || "Joueur"}</p>
            <p className="text-[0.68rem] text-warning">Hors-ligne</p>
          </div>
        </button>
      );
    }
    return (
      <>
        <Button onClick={() => setLoginOpen(true)} className="bg-gradient-brand shadow-glow w-full gap-2">
          <LogIn aria-hidden="true" />
          Se connecter
        </Button>
        <LoginDialog open={loginOpen} onOpenChange={setLoginOpen} />
      </>
    );
  }

  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <button className="flex w-full items-center gap-2.5 rounded-lg p-1.5 text-left transition-colors hover:bg-sidebar-accent">
          <img
            src={`https://mc-heads.net/avatar/${account.profile.id}/64`}
            alt=""
            className="size-8 rounded-md [image-rendering:pixelated]"
          />
          <div className="min-w-0 flex-1">
            <p className="truncate text-sm font-medium">{account.profile.name}</p>
            {account.offline ? (
              <p className="text-[0.68rem] text-warning">Hors connexion</p>
            ) : (
              <p className="text-[0.68rem] text-success">Compte Microsoft</p>
            )}
          </div>
          <ChevronUp className="size-3.5 text-sidebar-foreground/40" aria-hidden="true" />
        </button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="start" side="top" className="w-52">
        <DropdownMenuItem onClick={() => navigate("/settings?tab=account")} className="gap-2">
          <UserRound className="size-4" aria-hidden="true" />
          Mon compte
        </DropdownMenuItem>
        <DropdownMenuItem onClick={() => navigate("/skins")} className="gap-2">
          <Shirt className="size-4" aria-hidden="true" />
          Mon skin
        </DropdownMenuItem>
        {others.map((other) => (
          <DropdownMenuItem key={other.id} onClick={() => switchTo.mutate(other.id)} className="gap-2">
            <img
              src={`https://mc-heads.net/avatar/${other.id}/32`}
              alt=""
              className="size-4 rounded-sm [image-rendering:pixelated]"
            />
            {other.name}
          </DropdownMenuItem>
        ))}
        <DropdownMenuItem onClick={() => setLoginOpen(true)} className="gap-2">
          <UserPlus className="size-4" aria-hidden="true" />
          Ajouter un compte
        </DropdownMenuItem>
        <DropdownMenuSeparator />
        <DropdownMenuItem onClick={() => forget.mutate(account.profile.id)} className="gap-2 text-destructive">
          <LogOut className="size-4" aria-hidden="true" />
          Se déconnecter
        </DropdownMenuItem>
      </DropdownMenuContent>
      <LoginDialog open={loginOpen} onOpenChange={setLoginOpen} />
    </DropdownMenu>
  );
}

export function Sidebar() {
  const navigate = useNavigate();
  const version = useAppVersion();
  const runtime = useAppStore((s) => s.runtime);
  const { data: instances } = useQuery({ queryKey: ["instances"], queryFn: instancesApi.list });

  const running = (instances ?? []).filter((i) => runtime[i.id]?.running);
  const recent = [...(instances ?? [])]
    .filter((i) => !runtime[i.id]?.running)
    .sort((a, b) => (b.last_played_at ?? b.created_at) - (a.last_played_at ?? a.created_at))
    .slice(0, 5);

  return (
    <aside className="glass-strong z-10 flex w-60 shrink-0 flex-col border-y-0 border-l-0 text-sidebar-foreground">
      <div className="flex items-center gap-2.5 px-4 pt-4 pb-3">
        <motion.div
          whileHover={{ rotate: -8, scale: 1.08 }}
          transition={spring}
          className="bg-gradient-brand shadow-glow flex size-9 shrink-0 items-center justify-center rounded-xl text-base font-black text-primary-foreground"
        >
          L
        </motion.div>
        <div className="min-w-0 flex-1">
          <h1 className="text-sm leading-tight font-bold">Largy Launcher</h1>
          <p className="text-[0.68rem] text-sidebar-foreground/45">{version ? `v${version}` : "Minecraft"}</p>
        </div>
        <NotificationCenter />
      </div>

      <nav aria-label="Navigation principale" className="flex flex-col gap-0.5 px-2">
        {NAV_ITEMS.map(({ to, label, icon: Icon }) => (
          <NavLink key={to} to={to} end={to === "/"} className="relative">
            {({ isActive }) => (
              <span
                className={cn(
                  "relative flex items-center gap-2.5 rounded-lg px-2.5 py-2 text-sm transition-colors",
                  isActive
                    ? "font-semibold text-sidebar-foreground"
                    : "text-sidebar-foreground/60 hover:bg-sidebar-accent/70 hover:text-sidebar-foreground",
                )}
              >
                {isActive && (
                  <motion.span
                    layoutId="nav-active"
                    transition={spring}
                    className="absolute inset-0 rounded-lg bg-accent ring-1 ring-primary/25"
                  >
                    <span className="bg-gradient-brand absolute top-1.5 bottom-1.5 left-0 w-1 rounded-full" />
                  </motion.span>
                )}
                <Icon className={cn("relative size-4", isActive && "text-primary")} aria-hidden="true" />
                <span className="relative">{label}</span>
              </span>
            )}
          </NavLink>
        ))}
      </nav>

      <div className="flex-1 overflow-y-auto px-2 pb-2">
        {running.length > 0 && (
          <>
            <SectionTitle icon={LayoutGrid}>En cours</SectionTitle>
            {running.map((instance) => (
              <InstanceShortcut key={instance.id} instance={instance} />
            ))}
          </>
        )}
        {recent.length > 0 && (
          <>
            <SectionTitle icon={History}>Récentes</SectionTitle>
            {recent.map((instance) => (
              <InstanceShortcut key={instance.id} instance={instance} />
            ))}
          </>
        )}
      </div>

      <div className="space-y-1.5 border-t border-sidebar-border p-2.5">
        <button
          onClick={() => navigate("/settings?tab=appearance")}
          className="flex w-full items-center gap-2 rounded-lg px-2 py-1.5 text-xs text-sidebar-foreground/55 transition-colors hover:bg-sidebar-accent hover:text-sidebar-foreground"
        >
          <Palette className="size-3.5" aria-hidden="true" />
          Personnaliser l'apparence
        </button>
        <AccountCard />
      </div>
    </aside>
  );
}
