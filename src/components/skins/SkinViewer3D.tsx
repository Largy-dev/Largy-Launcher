import { useEffect, useRef, useState } from "react";
import { Loader2 } from "lucide-react";

import type { SkinVariant } from "@/services/skins";

type Viewer = import("skinview3d").SkinViewer;

interface SkinViewer3DProps {
  skin: string | null;
  cape: string | null;
  variant: SkinVariant;
  width: number;
  height: number;
  /** Walk instead of standing idle. */
  walking?: boolean;
}

/**
 * Rotatable 3D preview (drag to turn, scroll to zoom). skinview3d and
 * three.js are loaded on first use so they stay out of the main bundle.
 */
export function SkinViewer3D({ skin, cape, variant, width, height, walking = false }: SkinViewer3DProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const viewerRef = useRef<Viewer | null>(null);
  const libRef = useRef<typeof import("skinview3d") | null>(null);
  const [ready, setReady] = useState(false);

  useEffect(() => {
    let disposed = false;
    import("skinview3d").then((lib) => {
      if (disposed || !canvasRef.current) return;
      const viewer = new lib.SkinViewer({ canvas: canvasRef.current, width, height, zoom: 0.85, fov: 50 });
      viewer.controls.enablePan = false;
      viewer.playerObject.rotation.y = -0.45;
      libRef.current = lib;
      viewerRef.current = viewer;
      setReady(true);
    });
    return () => {
      disposed = true;
      viewerRef.current?.dispose();
      viewerRef.current = null;
    };
    // The canvas is created once; size changes go through setSize below.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    if (ready) viewerRef.current?.setSize(width, height);
  }, [ready, width, height]);

  useEffect(() => {
    const viewer = viewerRef.current;
    if (!ready || !viewer) return;
    if (skin) viewer.loadSkin(skin, { model: variant === "slim" ? "slim" : "default" }).catch(() => {});
    else viewer.loadSkin(null);
  }, [ready, skin, variant]);

  useEffect(() => {
    const viewer = viewerRef.current;
    if (!ready || !viewer) return;
    if (cape) viewer.loadCape(cape).catch(() => {});
    else viewer.loadCape(null);
  }, [ready, cape]);

  useEffect(() => {
    const viewer = viewerRef.current;
    const lib = libRef.current;
    if (!ready || !viewer || !lib) return;
    const animation = walking ? new lib.WalkingAnimation() : new lib.IdleAnimation();
    animation.speed = walking ? 0.7 : 1;
    viewer.animation = animation;
  }, [ready, walking]);

  return (
    <div className="relative" style={{ width, height }}>
      <canvas ref={canvasRef} className="cursor-grab active:cursor-grabbing" aria-label="Aperçu 3D du skin" />
      {!ready && (
        <Loader2
          className="absolute top-1/2 left-1/2 size-6 -translate-x-1/2 -translate-y-1/2 animate-spin text-muted-foreground"
          aria-hidden="true"
        />
      )}
    </div>
  );
}
