import { notify } from "@/lib/notify";
import type { InstanceInstallResult } from "@/services/tauri";
import { useAppStore, type PendingInstallWarnings } from "@/store/appStore";

/** Toast (and warnings dialog) after a modpack install, update or import. */
export function notifyInstallResult(result: InstanceInstallResult, title: string, openInstance: () => void) {
  const pending: PendingInstallWarnings = {
    instanceId: result.instance.id,
    instanceName: result.instance.name,
    warnings: result.warnings,
  };
  const { setInstallWarnings } = useAppStore.getState();
  if (result.warnings.length > 0) {
    notify.warning({
      title,
      message: `${result.warnings.length} fichier(s) n'ont pas pu être installés automatiquement.`,
      native: "installDone",
      sticky: true,
      action: { label: "Voir la liste", onClick: () => setInstallWarnings(pending) },
    });
    setInstallWarnings(pending);
  } else {
    notify.success({
      title,
      message: "Tout est prêt, bon jeu !",
      native: "installDone",
      action: { label: "Ouvrir", onClick: openInstance },
    });
  }
}
