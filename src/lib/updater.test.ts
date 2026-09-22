import { beforeEach, describe, expect, it, vi } from "vitest";

const { checkMock, relaunchMock } = vi.hoisted(() => ({ checkMock: vi.fn(), relaunchMock: vi.fn() }));

vi.mock("@tauri-apps/plugin-updater", () => ({ check: checkMock }));
vi.mock("@tauri-apps/plugin-process", () => ({ relaunch: relaunchMock }));

import { checkForAppUpdate, installAppUpdate } from "./updater";

beforeEach(() => {
  checkMock.mockReset();
  relaunchMock.mockReset();
});

describe("checkForAppUpdate", () => {
  it("delegates to the plugin's check()", async () => {
    checkMock.mockResolvedValue(null);
    const result = await checkForAppUpdate();
    expect(result).toBeNull();
    expect(checkMock).toHaveBeenCalled();
  });
});

describe("installAppUpdate", () => {
  it("reports cumulative percent progress and relaunches on finish", async () => {
    const onProgress = vi.fn();
    const downloadAndInstall = vi.fn(async (handler: (event: unknown) => void) => {
      handler({ event: "Started", data: { contentLength: 200 } });
      handler({ event: "Progress", data: { chunkLength: 50 } });
      handler({ event: "Progress", data: { chunkLength: 50 } });
      handler({ event: "Finished" });
    });

    await installAppUpdate({ downloadAndInstall } as never, onProgress);

    expect(onProgress).toHaveBeenNthCalledWith(1, 25);
    expect(onProgress).toHaveBeenNthCalledWith(2, 50);
    expect(onProgress).toHaveBeenNthCalledWith(3, 100);
    expect(relaunchMock).toHaveBeenCalled();
  });

  it("does not divide by zero when contentLength is unknown", async () => {
    const onProgress = vi.fn();
    const downloadAndInstall = vi.fn(async (handler: (event: unknown) => void) => {
      handler({ event: "Started", data: {} });
      handler({ event: "Progress", data: { chunkLength: 50 } });
    });

    await installAppUpdate({ downloadAndInstall } as never, onProgress);

    expect(onProgress).not.toHaveBeenCalledWith(expect.any(Number));
  });
});
