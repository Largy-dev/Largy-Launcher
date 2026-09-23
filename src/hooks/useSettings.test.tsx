import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { ReactNode } from "react";

vi.mock("@/services/tauri", () => ({ settingsApi: { get: vi.fn() } }));

import { settingsApi } from "@/services/tauri";

import { useSettings } from "./useSettings";

const getMock = vi.mocked(settingsApi.get);

function wrapper({ children }: { children: ReactNode }) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return <QueryClientProvider client={client}>{children}</QueryClientProvider>;
}

beforeEach(() => {
  getMock.mockReset();
});

describe("useSettings", () => {
  it("resolves with the settings returned by settingsApi.get", async () => {
    getMock.mockResolvedValue({
      default_min_memory_mb: 1024,
      default_max_memory_mb: 4096,
      default_jvm_args: [],
      azure_client_id: "id",
      curseforge_api_key: "",
      java_path_override: null,
      offline_mode: false,
      offline_username: "",
      on_game_launch: "keep_open",
    });

    const { result } = renderHook(() => useSettings(), { wrapper });

    await waitFor(() => expect(result.current.isSuccess).toBe(true));
    expect(result.current.data?.default_min_memory_mb).toBe(1024);
    expect(getMock).toHaveBeenCalledTimes(1);
  });
});
