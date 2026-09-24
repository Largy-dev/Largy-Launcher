import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { open } from "@tauri-apps/plugin-dialog";
import { Footprints, Loader2, RotateCcw, Shirt, Trash2, Upload, UserRound, X } from "lucide-react";

import { EmptyState } from "@/components/EmptyState";
import { PageHeader } from "@/components/PageHeader";
import { SettingSection } from "@/components/settings/SettingsKit";
import { SkinThumbnail } from "@/components/skins/SkinThumbnail";
import { SkinViewer3D } from "@/components/skins/SkinViewer3D";
import { Button } from "@/components/ui/button";
import { notify } from "@/lib/notify";
import { cn } from "@/lib/utils";
import { skinsApi, type Cape, type LibrarySkin, type SkinProfile, type SkinVariant } from "@/services/skins";
import { errorMessage } from "@/services/tauri";
import { useAppStore } from "@/store/appStore";

import { SkinImportDialog, type PickedSkin } from "./SkinImportDialog";

interface Preview {
  texture: string;
  variant: SkinVariant;
  libraryId: string;
  name: string;
}

/** Front of a cape (its 10×16 outer face), cropped from the 64×32 texture. */
function CapeFace({ texture }: { texture: string | null }) {
  const scale = 4;
  return (
    <div
      className="rounded-sm bg-muted [image-rendering:pixelated]"
      style={{
        width: 10 * scale,
        height: 16 * scale,
        backgroundImage: texture ? `url(${texture})` : undefined,
        backgroundSize: `${64 * scale}px ${32 * scale}px`,
        backgroundPosition: `-${scale}px -${scale}px`,
      }}
    />
  );
}

function CapeOption({
  label,
  cape,
  selected,
  disabled,
  onSelect,
}: {
  label: string;
  cape: Cape | null;
  selected: boolean;
  disabled: boolean;
  onSelect: () => void;
}) {
  return (
    <button
      onClick={onSelect}
      disabled={disabled || selected}
      aria-pressed={selected}
      className={cn(
        "flex w-24 flex-col items-center gap-2 rounded-xl p-3 text-xs ring-1 ring-border/60 transition",
        selected ? "bg-accent font-semibold ring-2 ring-primary" : "hover:bg-accent/60",
      )}
    >
      {cape ? (
        <CapeFace texture={cape.texture} />
      ) : (
        <div className="flex h-16 w-10 items-center justify-center rounded-sm bg-muted">
          <X className="size-4 text-muted-foreground" aria-hidden="true" />
        </div>
      )}
      <span className="w-full truncate text-center">{label}</span>
    </button>
  );
}

function LibraryCard({
  skin,
  previewing,
  onPreview,
  onRemove,
}: {
  skin: LibrarySkin;
  previewing: boolean;
  onPreview: () => void;
  onRemove: () => void;
}) {
  return (
    <div
      className={cn(
        "group relative flex flex-col items-center gap-2 rounded-xl p-3 ring-1 ring-border/60 transition",
        previewing ? "bg-accent ring-2 ring-primary" : "hover:bg-accent/60",
      )}
    >
      <button onClick={onPreview} className="flex flex-col items-center gap-2" aria-label={`Aperçu de ${skin.name}`}>
        <SkinThumbnail texture={skin.texture} variant={skin.variant} />
        <span className="w-24 truncate text-center text-xs font-medium">{skin.name}</span>
      </button>
      <Button
        variant="ghost"
        size="icon-xs"
        title="Retirer de la bibliothèque"
        aria-label={`Retirer ${skin.name}`}
        className="absolute top-1 right-1 opacity-0 transition-opacity group-hover:opacity-100 focus-visible:opacity-100"
        onClick={onRemove}
      >
        <Trash2 aria-hidden="true" />
      </Button>
    </div>
  );
}

export function SkinsScreen() {
  const queryClient = useQueryClient();
  const account = useAppStore((s) => s.account);
  const online = !!account && !account.offline;
  const [preview, setPreview] = useState<Preview | null>(null);
  const [walking, setWalking] = useState(false);
  const [picked, setPicked] = useState<PickedSkin | null>(null);

  const profileKey = ["skin-profile", account?.profile.id];
  const profile = useQuery({
    queryKey: profileKey,
    queryFn: skinsApi.profile,
    enabled: online,
    retry: false,
    staleTime: 60 * 1000,
  });
  const library = useQuery({ queryKey: ["skin-library"], queryFn: skinsApi.library });

  const onProfile = (title: string) => (next: SkinProfile) => {
    queryClient.setQueryData(profileKey, next);
    setPreview(null);
    notify.success({ title, message: "Visible en jeu à ta prochaine connexion.", history: false });
  };
  const onError = (title: string) => (e: unknown) => notify.error({ title, message: errorMessage(e) });

  const wear = useMutation({
    mutationFn: (id: string) => skinsApi.apply(id),
    onSuccess: onProfile("Skin changé"),
    onError: onError("Changement de skin impossible"),
  });
  const upload = useMutation({
    mutationFn: (s: { path: string; variant: SkinVariant; name: string }) => skinsApi.upload(s.path, s.variant, s.name),
    onSuccess: (next) => {
      setPicked(null);
      queryClient.invalidateQueries({ queryKey: ["skin-library"] });
      onProfile("Skin changé")(next);
    },
    onError: onError("Changement de skin impossible"),
  });
  const save = useMutation({
    mutationFn: (s: { path: string; variant: SkinVariant; name: string }) =>
      skinsApi.addToLibrary(s.path, s.variant, s.name),
    onSuccess: () => {
      setPicked(null);
      queryClient.invalidateQueries({ queryKey: ["skin-library"] });
    },
    onError: onError("Ajout impossible"),
  });
  const reset = useMutation({
    mutationFn: skinsApi.reset,
    onSuccess: onProfile("Skin par défaut rétabli"),
    onError: onError("Réinitialisation impossible"),
  });
  const cape = useMutation({
    mutationFn: (id: string | null) => skinsApi.setCape(id),
    onSuccess: onProfile("Cape changée"),
    onError: onError("Changement de cape impossible"),
  });
  const remove = useMutation({
    mutationFn: (id: string) => skinsApi.removeFromLibrary(id),
    onSuccess: (_, id) => {
      if (preview?.libraryId === id) setPreview(null);
      queryClient.invalidateQueries({ queryKey: ["skin-library"] });
    },
    onError: onError("Suppression impossible"),
  });

  async function pickFile() {
    const path = await open({ title: "Choisir un skin", filters: [{ name: "Skin Minecraft", extensions: ["png"] }] });
    if (typeof path !== "string") return;
    try {
      const texture = await skinsApi.readFile(path);
      const name =
        path
          .split(/[\\/]/)
          .pop()
          ?.replace(/\.png$/i, "") ?? "Skin";
      setPicked({ path, texture, name });
    } catch (e) {
      notify.error({ title: "Ce fichier n'est pas un skin", message: errorMessage(e), history: false });
    }
  }

  const current = profile.data;
  const activeCape = current?.capes.find((c) => c.active) ?? null;
  const busy = wear.isPending || upload.isPending || reset.isPending || cape.isPending;

  return (
    <div className="flex flex-1 flex-col">
      <PageHeader
        eyebrow="Apparence"
        title="Skins"
        description="Ton skin et ta cape, tels que les autres joueurs les voient."
        action={
          <Button size="sm" className="gap-1.5" onClick={pickFile}>
            <Upload aria-hidden="true" />
            Importer un skin
          </Button>
        }
      />

      <div className="flex flex-1 gap-6">
        <div className="glass sticky top-0 flex h-fit w-72 shrink-0 flex-col items-center gap-3 rounded-2xl p-4">
          {online && profile.isLoading ? (
            <div className="flex h-[380px] items-center justify-center">
              <Loader2 className="size-6 animate-spin text-muted-foreground" aria-hidden="true" />
            </div>
          ) : (
            <SkinViewer3D
              skin={preview?.texture ?? current?.skin ?? null}
              cape={preview ? null : (activeCape?.texture ?? null)}
              variant={preview?.variant ?? current?.variant ?? "classic"}
              width={240}
              height={380}
              walking={walking}
            />
          )}
          <div className="w-full text-center">
            <p className="font-semibold">{preview ? preview.name : (current?.name ?? account?.profile.name ?? "—")}</p>
            <p className="text-xs text-muted-foreground">
              {preview ? "Aperçu — pas encore porté" : current ? "Skin actuel" : ""}
            </p>
          </div>
          <div className="flex w-full flex-col gap-2">
            {preview ? (
              <>
                <Button
                  className="bg-gradient-brand gap-1.5"
                  disabled={!online || busy}
                  onClick={() => wear.mutate(preview.libraryId)}
                >
                  {wear.isPending ? (
                    <Loader2 className="animate-spin" aria-hidden="true" />
                  ) : (
                    <Shirt aria-hidden="true" />
                  )}
                  Porter ce skin
                </Button>
                <Button variant="outline" onClick={() => setPreview(null)}>
                  Revenir à mon skin
                </Button>
              </>
            ) : (
              <Button
                variant="ghost"
                size="sm"
                className="gap-1.5"
                disabled={!online || busy}
                onClick={() => reset.mutate()}
              >
                <RotateCcw aria-hidden="true" />
                Skin par défaut
              </Button>
            )}
            <Button variant="ghost" size="sm" className="gap-1.5" onClick={() => setWalking((w) => !w)}>
              <Footprints aria-hidden="true" />
              {walking ? "Arrêter de marcher" : "Marcher"}
            </Button>
          </div>
        </div>

        <div className="min-w-0 flex-1 pb-12">
          {!online && (
            <div className="mb-5">
              <EmptyState
                icon={UserRound}
                title={account ? "Serveurs Minecraft injoignables" : "Compte Microsoft requis"}
                description={
                  account
                    ? "Ton skin s'affichera dès que la connexion sera rétablie. Ta bibliothèque reste disponible."
                    : "Connecte un compte Microsoft pour voir et changer ton skin. Tu peux déjà préparer ta bibliothèque."
                }
              />
            </div>
          )}
          {profile.isError && (
            <p className="mb-5 rounded-xl bg-destructive/10 px-4 py-3 text-sm text-destructive">
              {errorMessage(profile.error)}
            </p>
          )}

          <SettingSection title="Ma bibliothèque">
            {library.data?.length ? (
              <div className="flex flex-wrap gap-3 p-4">
                {library.data.map((skin) => (
                  <LibraryCard
                    key={skin.id}
                    skin={skin}
                    previewing={preview?.libraryId === skin.id}
                    onPreview={() =>
                      setPreview({ texture: skin.texture, variant: skin.variant, libraryId: skin.id, name: skin.name })
                    }
                    onRemove={() => remove.mutate(skin.id)}
                  />
                ))}
              </div>
            ) : (
              <p className="px-4 py-6 text-center text-sm text-muted-foreground">
                Les skins que tu importes sont gardés ici pour en changer en un clic.
              </p>
            )}
          </SettingSection>

          {current && current.capes.length > 0 && (
            <SettingSection title="Cape">
              <div className="flex flex-wrap gap-3 p-4">
                <CapeOption
                  label="Aucune"
                  cape={null}
                  selected={!activeCape}
                  disabled={busy}
                  onSelect={() => cape.mutate(null)}
                />
                {current.capes.map((c) => (
                  <CapeOption
                    key={c.id}
                    label={c.alias || "Cape"}
                    cape={c}
                    selected={c.active}
                    disabled={busy}
                    onSelect={() => cape.mutate(c.id)}
                  />
                ))}
              </div>
            </SettingSection>
          )}
        </div>
      </div>

      <SkinImportDialog
        picked={picked}
        onOpenChange={(isOpen) => !isOpen && setPicked(null)}
        pending={upload.isPending || save.isPending}
        canWear={online}
        onConfirm={(skin, wearNow) => (wearNow ? upload.mutate(skin) : save.mutate(skin))}
      />
    </div>
  );
}
