import { useState } from "react";
import { NavLink, useMatch, useNavigate } from "react-router";
import { motion } from "motion/react";
import {
  ChevronDown,
  Compass,
  House,
  LogIn,
  LogOut,
  Search,
  Server,
  Settings,
  Shirt,
  UserPlus,
  UserRound,
} from "lucide-react";

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
import { spring } from "@/lib/motion";
import { cn } from "@/lib/utils";
import { LoginDialog } from "@/screens/Login/LoginScreen";
import { useAppStore } from "@/store/appStore";

import { openCommandPalette } from "./CommandPalette";
import { NotificationCenter } from "./NotificationCenter";

const NAV_ITEMS = [
  { to: "/", label: "Accueil", icon: House },
  { to: "/modpacks", label: "Modpacks", icon: Compass },
  { to: "/servers", label: "Serveurs", icon: Server },
  { to: "/skins", label: "Skins", icon: Shirt },
];

function NavPill({
  to,
  label,
  icon: Icon,
  active,
}: {
  to: string;
  label: string;
  icon: typeof House;
  active: boolean;
}) {
  return (
    <NavLink
      to={to}
      className={cn(
        "relative flex items-center gap-1.5 rounded-lg px-3.5 py-1.5 text-sm transition-colors",
        active ? "font-semibold text-foreground" : "text-foreground/60 hover:text-foreground",
      )}
    >
      {active && (
        <motion.span
          layoutId="topnav-active"
          transition={spring}
          className="absolute inset-0 rounded-lg bg-foreground/10 ring-1 ring-foreground/10"
        />
      )}
      <Icon className="relative size-3.5 lg:hidden" aria-hidden="true" />
      <span className="relative max-lg:sr-only">{label}</span>
    </NavLink>
  );
}

function AccountMenu() {
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
          className="tile flex items-center gap-2.5 py-1 pr-3 pl-1 text-left transition-colors hover:bg-[var(--surface-hover)]"
        >
          <span className="flex size-8 items-center justify-center rounded-lg bg-muted">
            <UserRound className="size-4 text-muted-foreground" aria-hidden="true" />
          </span>
          <span className="min-w-0">
            <span className="block max-w-32 truncate text-sm font-semibold">
              {settings.offline_username.trim() || "Steve"}
            </span>
            <span className="block text-[0.68rem] text-warning">Hors-ligne</span>
          </span>
        </button>
      );
    }
    return (
      <>
        <Button onClick={() => setLoginOpen(true)} className="bg-gradient-brand shadow-glow h-10 gap-2 px-4">
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
        <button className="tile flex items-center gap-2.5 py-1 pr-2.5 pl-1 text-left transition-colors hover:bg-[var(--surface-hover)]">
          <img
            src={`https://mc-heads.net/avatar/${account.profile.id}/64`}
            alt=""
            className="size-8 rounded-lg [image-rendering:pixelated]"
          />
          <span className="min-w-0">
            <span className="block max-w-32 truncate text-sm font-semibold">{account.profile.name}</span>
            <span className={cn("block text-[0.68rem]", account.offline ? "text-warning" : "text-muted-foreground")}>
              {account.offline ? "Hors connexion" : "Compte Microsoft"}
            </span>
          </span>
          <ChevronDown className="size-3.5 text-muted-foreground" aria-hidden="true" />
        </button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end" className="w-56">
        <DropdownMenuItem onClick={() => navigate("/settings?tab=account")} className="gap-2">
          <UserRound className="size-4" aria-hidden="true" />
          Mon compte
        </DropdownMenuItem>
        <DropdownMenuItem onClick={() => navigate("/skins")} className="gap-2">
          <Shirt className="size-4" aria-hidden="true" />
          Mon skin
        </DropdownMenuItem>
        {others.length > 0 && <DropdownMenuSeparator />}
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

/** Section tabs on the left; search, notifications, account and settings on the right. */
export function TopBar() {
  const navigate = useNavigate();
  const atRoot = useMatch("/");
  const atInstance = useMatch("/instances/:id");
  const atModpacks = useMatch("/modpacks");
  const atServers = useMatch("/servers");
  const atSkins = useMatch("/skins");
  const atSettings = useMatch("/settings");
  const active: Record<string, boolean> = {
    "/": atRoot !== null || atInstance !== null,
    "/modpacks": atModpacks !== null,
    "/servers": atServers !== null,
    "/skins": atSkins !== null,
  };

  return (
    <header className="flex items-center gap-3 px-6 pt-4 pb-2">
      <nav aria-label="Navigation principale" className="tile flex items-center gap-0.5 p-1">
        {NAV_ITEMS.map((item) => (
          <NavPill key={item.to} {...item} active={active[item.to]} />
        ))}
      </nav>
      <div className="flex-1" />
      <button
        onClick={openCommandPalette}
        className="tile flex h-10 items-center gap-2 px-3 text-sm text-muted-foreground transition-colors hover:text-foreground"
        title="Recherche rapide (Ctrl+K)"
      >
        <Search className="size-4" aria-hidden="true" />
        <kbd className="hidden rounded bg-foreground/8 px-1.5 font-mono text-[0.68rem] xl:inline">Ctrl K</kbd>
      </button>
      <div className="tile flex size-10 items-center justify-center">
        <NotificationCenter />
      </div>
      <AccountMenu />
      <button
        onClick={() => navigate("/settings")}
        title="Paramètres"
        aria-label="Paramètres"
        aria-current={atSettings ? "page" : undefined}
        className={cn(
          "tile flex size-10 items-center justify-center text-muted-foreground transition-colors hover:text-foreground",
          atSettings && "text-primary ring-1 ring-primary/40",
        )}
      >
        <Settings className="size-4" aria-hidden="true" />
      </button>
    </header>
  );
}
