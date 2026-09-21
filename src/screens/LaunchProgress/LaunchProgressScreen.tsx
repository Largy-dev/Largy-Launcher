import { useEffect, useRef } from "react";
import { useParams } from "react-router";
import { Loader2, Square } from "lucide-react";
import { toast } from "sonner";

import { Button } from "@/components/ui/button";
import { PageHeader } from "@/components/PageHeader";
import { Progress } from "@/components/ui/progress";
import { ScrollArea } from "@/components/ui/scroll-area";
import { errorMessage, launchApi } from "@/services/tauri";
import { useAppStore } from "@/store/appStore";

export function LaunchProgressScreen() {
  const { id } = useParams<{ id: string }>();
  const instanceId = id ?? "";
  const running = useAppStore((s) => s.runtime[instanceId]?.running ?? false);
  const logs = useAppStore((s) => s.runtime[instanceId]?.logs ?? []);
  const progress = useAppStore((s) => s.downloadProgress);
  const scrollRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    scrollRef.current?.scrollIntoView({ block: "end" });
  }, [logs]);

  async function stop() {
    try {
      await launchApi.stop(instanceId);
    } catch (e) {
      toast.error(errorMessage(e));
    }
  }

  const percent =
    progress && progress.bytes_total > 0
      ? Math.min(100, Math.round((progress.bytes_done / progress.bytes_total) * 100))
      : null;

  return (
    <div className="flex flex-1 flex-col">
      <PageHeader
        title="Lancement en cours"
        description={instanceId}
        action={
          running ? (
            <Button variant="destructive" size="sm" onClick={stop} className="gap-1.5">
              <Square className="size-3.5" aria-hidden="true" />
              Arrêter
            </Button>
          ) : undefined
        }
      />

      {running && percent !== null && (
        <div className="mb-4 space-y-1.5">
          <div className="flex justify-between text-xs text-muted-foreground">
            <span>{progress?.label}</span>
            <span>
              {progress?.files_done}/{progress?.files_total} — {percent}%
            </span>
          </div>
          <Progress value={percent} />
        </div>
      )}

      <ScrollArea className="flex-1 rounded-lg border border-border bg-card">
        <div className="p-3 font-mono text-xs leading-relaxed">
          {logs.length === 0 ? (
            <p className="flex items-center gap-2 text-muted-foreground">
              <Loader2 className="size-3.5 animate-spin" aria-hidden="true" />
              En attente des premiers logs…
            </p>
          ) : (
            logs.map((line, i) => (
              <p key={i} className="whitespace-pre-wrap break-all text-foreground/80">
                {line}
              </p>
            ))
          )}
          <div ref={scrollRef} />
        </div>
      </ScrollArea>
    </div>
  );
}
