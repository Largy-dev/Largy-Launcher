import { useEffect, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { convertFileSrc } from "@tauri-apps/api/core";
import { ChevronLeft, ChevronRight, ClipboardCopy, FolderOpen, Images, Loader2, Trash2 } from "lucide-react";

import { ConfirmDialog } from "@/components/ConfirmDialog";
import { EmptyState } from "@/components/EmptyState";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogDescription, DialogTitle } from "@/components/ui/dialog";
import { formatBytes, formatRelative } from "@/lib/format";
import { notify } from "@/lib/notify";
import { errorMessage, instancesApi, type Instance, type Screenshot } from "@/services/tauri";

/** Puts the image on the clipboard as PNG (the only image type browsers write). */
async function copyImage(url: string) {
  const blob = await (await fetch(url)).blob();
  let png = blob;
  if (blob.type !== "image/png") {
    const bitmap = await createImageBitmap(blob);
    const canvas = document.createElement("canvas");
    canvas.width = bitmap.width;
    canvas.height = bitmap.height;
    canvas.getContext("2d")?.drawImage(bitmap, 0, 0);
    png = await new Promise<Blob>((resolve, reject) =>
      canvas.toBlob((b) => (b ? resolve(b) : reject(new Error("conversion impossible"))), "image/png"),
    );
  }
  await navigator.clipboard.write([new ClipboardItem({ "image/png": png })]);
}

/** The game's screenshots folder as a gallery with a full-size viewer. */
export function ScreenshotsTab({ instance }: { instance: Instance }) {
  const queryClient = useQueryClient();
  const queryKey = ["instance-screenshots", instance.id];
  const { data: shots, isLoading } = useQuery({ queryKey, queryFn: () => instancesApi.screenshots(instance.id) });
  const [openIndex, setOpenIndex] = useState<number | null>(null);
  const [deleting, setDeleting] = useState<Screenshot | null>(null);
  const current = openIndex !== null ? shots?.[openIndex] : undefined;
  const count = shots?.length ?? 0;

  const remove = useMutation({
    mutationFn: (shot: Screenshot) => instancesApi.deleteScreenshot(instance.id, shot.file_name),
    onSuccess: () => {
      setDeleting(null);
      if (openIndex !== null && openIndex >= count - 1) setOpenIndex(count > 1 ? count - 2 : null);
      queryClient.invalidateQueries({ queryKey });
    },
    onError: (e) => notify.error({ title: "Suppression impossible", message: errorMessage(e), history: false }),
  });

  useEffect(() => {
    if (openIndex === null || count === 0) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "ArrowRight") setOpenIndex((i) => (i === null ? i : (i + 1) % count));
      if (e.key === "ArrowLeft") setOpenIndex((i) => (i === null ? i : (i - 1 + count) % count));
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [openIndex, count]);

  async function copy(shot: Screenshot) {
    try {
      await copyImage(convertFileSrc(shot.path));
      notify.success({ title: "Capture copiée", message: "Colle-la où tu veux avec Ctrl+V.", history: false });
    } catch (e) {
      notify.error({ title: "Copie impossible", message: errorMessage(e), history: false });
    }
  }

  if (isLoading) {
    return (
      <div className="flex justify-center py-12">
        <Loader2 className="size-5 animate-spin text-muted-foreground" aria-hidden="true" />
      </div>
    );
  }

  return (
    <>
      <div className="mb-3 flex items-center justify-between">
        <p className="text-sm text-muted-foreground">
          {count === 0 ? "" : `${count} capture${count > 1 ? "s" : ""} — F2 en jeu pour en prendre une.`}
        </p>
        <Button
          variant="outline"
          size="sm"
          className="gap-1.5"
          onClick={() => instancesApi.openFolder(instance.id, "screenshots")}
        >
          <FolderOpen aria-hidden="true" />
          Ouvrir le dossier
        </Button>
      </div>

      {count === 0 ? (
        <EmptyState
          icon={Images}
          title="Aucune capture d'écran"
          description="Appuie sur F2 pendant une partie : tes captures apparaîtront ici."
        />
      ) : (
        <div className="grid grid-cols-2 gap-3 lg:grid-cols-3">
          {shots!.map((shot, index) => (
            <button
              key={shot.file_name}
              onClick={() => setOpenIndex(index)}
              className="group relative aspect-video overflow-hidden rounded-xl bg-muted ring-1 ring-border/60 transition hover:ring-primary/60 focus-visible:ring-2 focus-visible:ring-primary focus-visible:outline-none"
            >
              <img
                src={convertFileSrc(shot.path)}
                alt={shot.file_name}
                loading="lazy"
                decoding="async"
                className="size-full object-cover transition-transform duration-300 group-hover:scale-105"
              />
              <span className="absolute inset-x-0 bottom-0 bg-gradient-to-t from-black/70 to-transparent px-2.5 pt-6 pb-1.5 text-left text-[0.7rem] text-white/90 opacity-0 transition-opacity group-hover:opacity-100">
                {formatRelative(shot.taken_at)}
              </span>
            </button>
          ))}
        </div>
      )}

      <Dialog open={!!current} onOpenChange={(open) => !open && setOpenIndex(null)}>
        <DialogContent className="max-w-[min(92vw,1200px)] gap-3 p-3 sm:max-w-[min(92vw,1200px)]">
          {current && (
            <>
              <div className="relative overflow-hidden rounded-lg bg-black">
                <img
                  src={convertFileSrc(current.path)}
                  alt={current.file_name}
                  className="mx-auto max-h-[72vh] w-auto object-contain"
                />
                {count > 1 && (
                  <>
                    <Button
                      variant="secondary"
                      size="icon"
                      aria-label="Capture précédente"
                      className="absolute top-1/2 left-2 -translate-y-1/2 rounded-full opacity-80 hover:opacity-100"
                      onClick={() => setOpenIndex((openIndex! - 1 + count) % count)}
                    >
                      <ChevronLeft aria-hidden="true" />
                    </Button>
                    <Button
                      variant="secondary"
                      size="icon"
                      aria-label="Capture suivante"
                      className="absolute top-1/2 right-2 -translate-y-1/2 rounded-full opacity-80 hover:opacity-100"
                      onClick={() => setOpenIndex((openIndex! + 1) % count)}
                    >
                      <ChevronRight aria-hidden="true" />
                    </Button>
                  </>
                )}
              </div>
              <div className="flex items-center justify-between gap-3 px-1">
                <div className="min-w-0">
                  <DialogTitle className="truncate text-sm">{current.file_name}</DialogTitle>
                  <DialogDescription className="text-xs">
                    {formatRelative(current.taken_at)} · {formatBytes(current.size)} · {openIndex! + 1}/{count}
                  </DialogDescription>
                </div>
                <div className="flex shrink-0 gap-2">
                  <Button variant="outline" size="sm" className="gap-1.5" onClick={() => copy(current)}>
                    <ClipboardCopy aria-hidden="true" />
                    Copier
                  </Button>
                  <Button
                    variant="outline"
                    size="sm"
                    className="gap-1.5"
                    onClick={() => instancesApi.revealFile(instance.id, current.path)}
                  >
                    <FolderOpen aria-hidden="true" />
                    Afficher
                  </Button>
                  <Button variant="destructive" size="sm" className="gap-1.5" onClick={() => setDeleting(current)}>
                    <Trash2 aria-hidden="true" />
                    Supprimer
                  </Button>
                </div>
              </div>
            </>
          )}
        </DialogContent>
      </Dialog>

      <ConfirmDialog
        open={deleting !== null}
        onOpenChange={(open) => !open && setDeleting(null)}
        title="Supprimer cette capture ?"
        description="Le fichier sera supprimé définitivement."
        confirmLabel="Supprimer"
        destructive
        pending={remove.isPending}
        onConfirm={() => deleting && remove.mutate(deleting)}
      />
    </>
  );
}
