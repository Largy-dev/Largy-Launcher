import { invoke } from "@tauri-apps/api/core";

export type SkinVariant = "classic" | "slim";

export interface Cape {
  id: string;
  alias: string;
  active: boolean;
  /** Data URL of the cape texture. */
  texture: string | null;
}

export interface SkinProfile {
  name: string;
  variant: SkinVariant;
  /** Data URL of the active skin; null for the default skin. */
  skin: string | null;
  capes: Cape[];
}

export interface LibrarySkin {
  id: string;
  name: string;
  variant: SkinVariant;
  added_at: number;
  texture: string;
}

export const skinsApi = {
  profile: () => invoke<SkinProfile>("skins_get_profile"),
  /** Uploads a PNG from disk; a copy is kept in the library. */
  upload: (path: string, variant: SkinVariant, name: string) =>
    invoke<SkinProfile>("skins_upload", { path, variant, name }),
  reset: () => invoke<SkinProfile>("skins_reset"),
  /** Shows a cape, or hides them all with null. */
  setCape: (capeId: string | null) => invoke<SkinProfile>("skins_set_cape", { capeId }),
  /** A skin file as a data URL, validated, for the preview before upload. */
  readFile: (path: string) => invoke<string>("skins_read_file", { path }),
  library: () => invoke<LibrarySkin[]>("skins_library_list"),
  addToLibrary: (path: string, variant: SkinVariant, name: string) =>
    invoke<LibrarySkin>("skins_library_add", { path, variant, name }),
  removeFromLibrary: (id: string) => invoke<void>("skins_library_remove", { id }),
  apply: (id: string) => invoke<SkinProfile>("skins_library_apply", { id }),
};
