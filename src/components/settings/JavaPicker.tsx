import { useQuery } from "@tanstack/react-query";
import { open } from "@tauri-apps/plugin-dialog";
import { FolderOpen, RefreshCw } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { notify } from "@/lib/notify";
import { errorMessage, javaApi } from "@/services/tauri";

const AUTO = "__auto__";

interface JavaPickerProps {
  /** `null` = let the launcher pick (or inherit the global choice). */
  value: string | null;
  onChange: (path: string | null) => void;
  autoLabel: string;
}

/** Chooses a Java among the ones found on this PC, a manual path, or automatic. */
export function JavaPicker({ value, onChange, autoLabel }: JavaPickerProps) {
  const {
    data: installs = [],
    isFetching,
    refetch,
  } = useQuery({
    queryKey: ["java-installations"],
    queryFn: javaApi.list,
    staleTime: 5 * 60_000,
  });
  const known = value !== null && installs.some((j) => j.path === value);

  async function browse() {
    const picked = await open({
      multiple: false,
      directory: false,
      filters: [{ name: "Java", extensions: ["exe"] }],
      title: "Choisir java.exe",
    });
    if (typeof picked !== "string") return;
    try {
      const probed = await javaApi.probe(picked);
      onChange(probed.path);
      notify.success({ title: `Java ${probed.version} sélectionné`, history: false });
    } catch (e) {
      notify.error({ title: "Java invalide", message: errorMessage(e), history: false });
    }
  }

  return (
    <div className="flex items-center gap-2">
      <Select value={value ?? AUTO} onValueChange={(v) => onChange(v === AUTO ? null : v)}>
        <SelectTrigger className="w-80">
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          <SelectItem value={AUTO}>{autoLabel}</SelectItem>
          {installs.map((j) => (
            <SelectItem key={j.path} value={j.path}>
              Java {j.major} · {j.version}
              {j.source === "managed" ? " (launcher)" : ""}
            </SelectItem>
          ))}
          {value !== null && !known && <SelectItem value={value}>{value}</SelectItem>}
        </SelectContent>
      </Select>
      <Button variant="outline" size="icon-sm" title="Parcourir…" aria-label="Parcourir" onClick={browse}>
        <FolderOpen aria-hidden="true" />
      </Button>
      <Button
        variant="ghost"
        size="icon-sm"
        title="Rechercher à nouveau"
        aria-label="Rechercher les Java installés"
        disabled={isFetching}
        onClick={() => refetch()}
      >
        <RefreshCw className={isFetching ? "animate-spin" : undefined} aria-hidden="true" />
      </Button>
    </div>
  );
}
