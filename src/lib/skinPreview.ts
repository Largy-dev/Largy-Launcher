import type { SkinVariant } from "@/services/skins";

/** One rectangle copied from the skin texture onto the front view. */
export interface SkinPart {
  sx: number;
  sy: number;
  w: number;
  h: number;
  dx: number;
  dy: number;
  /** Mirror horizontally (legacy 64×32 skins reuse the right limbs). */
  flip?: boolean;
}

/** Size of the front view, in skin pixels. */
export const FRONT_WIDTH = 16;
export const FRONT_HEIGHT = 32;

/**
 * Where each visible face of the body sits on the texture and on the front
 * view — base layer first, then the overlay (hat, jacket, sleeves…).
 * Legacy 64×32 skins have no overlay except the hat, and mirror the right
 * arm and leg for the left ones.
 */
export function frontParts(variant: SkinVariant, legacy: boolean): SkinPart[] {
  const arm = variant === "slim" ? 3 : 4;
  const base: SkinPart[] = [
    { sx: 8, sy: 8, w: 8, h: 8, dx: 4, dy: 0 },
    { sx: 20, sy: 20, w: 8, h: 12, dx: 4, dy: 8 },
    { sx: 44, sy: 20, w: arm, h: 12, dx: 4 - arm, dy: 8 },
    { sx: 4, sy: 20, w: 4, h: 12, dx: 4, dy: 20 },
    legacy
      ? { sx: 44, sy: 20, w: arm, h: 12, dx: 12, dy: 8, flip: true }
      : { sx: 36, sy: 52, w: arm, h: 12, dx: 12, dy: 8 },
    legacy ? { sx: 4, sy: 20, w: 4, h: 12, dx: 8, dy: 20, flip: true } : { sx: 20, sy: 52, w: 4, h: 12, dx: 8, dy: 20 },
  ];
  const hat: SkinPart = { sx: 40, sy: 8, w: 8, h: 8, dx: 4, dy: 0 };
  if (legacy) return [...base, hat];
  return [
    ...base,
    hat,
    { sx: 20, sy: 36, w: 8, h: 12, dx: 4, dy: 8 },
    { sx: 44, sy: 36, w: arm, h: 12, dx: 4 - arm, dy: 8 },
    { sx: 52, sy: 52, w: arm, h: 12, dx: 12, dy: 8 },
    { sx: 4, sy: 36, w: 4, h: 12, dx: 4, dy: 20 },
    { sx: 4, sy: 52, w: 4, h: 12, dx: 8, dy: 20 },
  ];
}

function loadImage(src: string): Promise<HTMLImageElement> {
  return new Promise((resolve, reject) => {
    const image = new Image();
    image.onload = () => resolve(image);
    image.onerror = () => reject(new Error("image illisible"));
    image.src = src;
  });
}

/** Flat front view of a skin as a small PNG data URL (scale it with `image-rendering: pixelated`). */
export async function renderSkinFront(texture: string, variant: SkinVariant): Promise<string> {
  const image = await loadImage(texture);
  const canvas = document.createElement("canvas");
  canvas.width = FRONT_WIDTH;
  canvas.height = FRONT_HEIGHT;
  const ctx = canvas.getContext("2d");
  if (!ctx) return texture;
  ctx.imageSmoothingEnabled = false;
  for (const p of frontParts(variant, image.height === 32)) {
    ctx.save();
    if (p.flip) {
      ctx.translate(p.dx + p.w, p.dy);
      ctx.scale(-1, 1);
      ctx.drawImage(image, p.sx, p.sy, p.w, p.h, 0, 0, p.w, p.h);
    } else {
      ctx.drawImage(image, p.sx, p.sy, p.w, p.h, p.dx, p.dy, p.w, p.h);
    }
    ctx.restore();
  }
  return canvas.toDataURL("image/png");
}

/**
 * Guesses the arm width: slim skins leave the 4th column of the right arm
 * (top face and front) fully transparent.
 */
export async function detectVariant(texture: string): Promise<SkinVariant> {
  const image = await loadImage(texture);
  if (image.height !== 64) return "classic";
  const canvas = document.createElement("canvas");
  canvas.width = 64;
  canvas.height = 64;
  const ctx = canvas.getContext("2d", { willReadFrequently: true });
  if (!ctx) return "classic";
  ctx.drawImage(image, 0, 0);
  const transparent = (x: number, y: number, w: number, h: number) =>
    ctx.getImageData(x, y, w, h).data.every((value, i) => i % 4 !== 3 || value === 0);
  return transparent(50, 16, 2, 4) && transparent(54, 20, 2, 12) ? "slim" : "classic";
}
