import { useState } from "react";
import { useMutation } from "@tanstack/react-query";
import { Link2, Loader2, Server } from "lucide-react";

import { useLaunchInstance } from "@/hooks/useLaunchInstance";
import { notify } from "@/lib/notify";
import { cn } from "@/lib/utils";
import { ModpackDetailDialog } from "@/screens/ModpackBrowser/ModpackDetailDialog";
import { errorMessage, providersApi, type Instance, type ModpackSummary } from "@/services/tauri";

/** `play.example.fr`, `play.example.fr:25566`, `192.168.1.20`… */
const SERVER_RE = /^[a-z0-9]([a-z0-9-]*[a-z0-9])?(\.[a-z0-9]([a-z0-9-]*[a-z0-9])?)*(:\d{1,5})?$/i;
const LINK_RE = /^https?:\/\//i;

export type QuickJoinTarget = { kind: "link"; url: string } | { kind: "server"; address: string } | null;

/** What the player pasted: a pack link, a server address, or nothing usable yet. */
export function parseQuickJoin(raw: string): QuickJoinTarget {
  const value = raw.trim();
  if (!value) return null;
  if (LINK_RE.test(value)) return { kind: "link", url: value };
  if (SERVER_RE.test(value) && value.includes(".")) return { kind: "server", address: value.toLowerCase() };
  return null;
}

/**
 * One field for both ways of joining friends: a server address launches this
 * instance straight onto it, a `.mrpack` link installs the shared pack.
 */
export function QuickJoin({ instance }: { instance: Instance }) {
  const [value, setValue] = useState("");
  const [pack, setPack] = useState<ModpackSummary | null>(null);
  const { play, running } = useLaunchInstance(instance);
  const target = parseQuickJoin(value);

  const checkLink = useMutation({
    mutationFn: (url: string) => providersApi.getModpack("url", url),
    onSuccess: (details) => {
      setValue("");
      setPack(details.summary);
    },
    onError: (e) => notify.error({ title: "Lien de pack invalide", message: errorMessage(e), history: false }),
  });

  function submit() {
    if (!target) {
      notify.warning({
        title: "Adresse non reconnue",
        message: "Colle l'adresse d'un serveur (play.exemple.fr) ou le lien d'un fichier .mrpack.",
        history: false,
      });
      return;
    }
    if (target.kind === "link") {
      checkLink.mutate(target.url);
    } else {
      setValue("");
      void play(target.address);
    }
  }

  const Icon = target?.kind === "link" ? Link2 : Server;

  return (
    <>
      <form
        onSubmit={(e) => {
          e.preventDefault();
          submit();
        }}
        className="tile flex h-14 min-w-0 flex-1 basis-72 items-center gap-2 pr-2 pl-4 focus-within:ring-2 focus-within:ring-ring/40 xl:max-w-md"
      >
        <Icon className="size-4 shrink-0 text-muted-foreground" aria-hidden="true" />
        <input
          value={value}
          onChange={(e) => setValue(e.target.value)}
          placeholder="Adresse d'un serveur ou lien d'un pack"
          aria-label="Adresse d'un serveur ou lien d'un pack partagé"
          className="min-w-0 flex-1 bg-transparent text-sm outline-none placeholder:text-muted-foreground"
        />
        <button
          type="submit"
          disabled={!value.trim() || checkLink.isPending || (target?.kind === "server" && running)}
          className={cn(
            "flex h-10 items-center gap-1.5 rounded-lg px-3.5 text-sm font-medium transition-colors disabled:opacity-50",
            target
              ? "bg-primary text-primary-foreground hover:bg-primary/85"
              : "bg-foreground/8 hover:bg-foreground/12",
          )}
        >
          {checkLink.isPending && <Loader2 className="size-3.5 animate-spin" aria-hidden="true" />}
          {target?.kind === "link" ? "Installer" : "Rejoindre"}
        </button>
      </form>
      <ModpackDetailDialog provider="url" pack={pack} onOpenChange={(o) => !o && setPack(null)} />
    </>
  );
}
