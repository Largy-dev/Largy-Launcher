import { useMutation } from "@tanstack/react-query";
import { useNavigate } from "react-router";
import { open } from "@tauri-apps/plugin-dialog";

import { notifyInstallResult } from "@/lib/installResult";
import { notify } from "@/lib/notify";
import { errorMessage, instancesApi, isCancelled } from "@/services/tauri";

export const IMPORTABLE_EXTENSIONS = ["mrpack", "zip"];

export function isImportable(path: string): boolean {
  const lower = path.toLowerCase();
  return IMPORTABLE_EXTENSIONS.some((ext) => lower.endsWith(`.${ext}`));
}

/** Imports `.mrpack`, CurseForge zips and Prism/MultiMC exports as new instances. */
export function useImportInstance() {
  const navigate = useNavigate();

  const importMutation = useMutation({
    mutationFn: (path: string) => instancesApi.import(path),
    onMutate: () => notify.info({ title: "Import en cours…", history: false }),
    onSuccess: (result) =>
      notifyInstallResult(result, `${result.instance.name} importé`, () =>
        navigate(`/instances/${result.instance.id}`),
      ),
    onError: (e) => {
      if (!isCancelled(e)) notify.error({ title: "Import impossible", message: errorMessage(e) });
    },
  });

  async function pickAndImport() {
    const picked = await open({
      multiple: false,
      title: "Importer un modpack ou une instance",
      filters: [{ name: "Modpack (.mrpack, .zip)", extensions: IMPORTABLE_EXTENSIONS }],
    });
    if (typeof picked === "string") importMutation.mutate(picked);
  }

  function importPaths(paths: string[]) {
    const files = paths.filter(isImportable);
    if (files.length === 0) {
      notify.warning({ title: "Glisse un fichier .mrpack ou .zip pour l'importer", history: false });
      return;
    }
    files.forEach((path) => importMutation.mutate(path));
  }

  return { pickAndImport, importPaths, importing: importMutation.isPending };
}
