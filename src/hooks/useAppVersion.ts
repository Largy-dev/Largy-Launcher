import { useEffect, useState } from "react";

import { getAppVersion } from "@/services/tauri";

export function useAppVersion(): string | null {
  const [version, setVersion] = useState<string | null>(null);

  useEffect(() => {
    getAppVersion()
      .then(setVersion)
      .catch(() => setVersion(null));
  }, []);

  return version;
}
