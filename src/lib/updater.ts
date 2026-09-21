import { check, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";

export type { Update };

/** `null` when already up to date. */
export function checkForAppUpdate(): Promise<Update | null> {
  return check();
}

/** Downloads, installs, and relaunches the app on the new version. */
export async function installAppUpdate(update: Update, onProgress?: (percent: number) => void): Promise<void> {
  let downloaded = 0;
  let contentLength = 0;

  await update.downloadAndInstall((event) => {
    switch (event.event) {
      case "Started":
        contentLength = event.data.contentLength ?? 0;
        break;
      case "Progress":
        downloaded += event.data.chunkLength;
        if (contentLength > 0) onProgress?.(Math.round((downloaded / contentLength) * 100));
        break;
      case "Finished":
        onProgress?.(100);
        break;
    }
  });

  await relaunch();
}
