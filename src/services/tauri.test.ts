import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn() }));

import { invoke } from "@tauri-apps/api/core";

import { auth, instancesApi, launchApi, minecraftApi, providersApi, settingsApi } from "./tauri";

const invokeMock = vi.mocked(invoke);

beforeEach(() => {
  invokeMock.mockReset();
  invokeMock.mockResolvedValue(undefined);
});

describe("auth", () => {
  it("beginLogin calls auth_begin_login with no args", async () => {
    await auth.beginLogin();
    expect(invokeMock).toHaveBeenCalledWith("auth_begin_login");
  });

  it("completeLogin passes the device code payload", async () => {
    const device = { device_code: "d", user_code: "u", verification_uri: "v", expires_in: 1, interval: 1 };
    await auth.completeLogin(device);
    expect(invokeMock).toHaveBeenCalledWith("auth_complete_login", { device });
  });

  it("logout forgets the active account by default, or the given one", async () => {
    await auth.logout();
    expect(invokeMock).toHaveBeenCalledWith("auth_logout", { accountId: null });
    await auth.logout("abc");
    expect(invokeMock).toHaveBeenCalledWith("auth_logout", { accountId: "abc" });
  });

  it("switchAccount camelCases accountId", async () => {
    await auth.switchAccount("abc");
    expect(invokeMock).toHaveBeenCalledWith("auth_switch_account", { accountId: "abc" });
  });
});

describe("settingsApi", () => {
  it("update sends the full settings object", async () => {
    const settings = {
      default_min_memory_mb: 1024,
      default_max_memory_mb: 4096,
      default_jvm_args: [],
      azure_client_id: "id",
      curseforge_api_key: "",
      java_path_override: null,
      offline_mode: false,
      offline_username: "",
      on_game_launch: "keep_open" as const,
      on_close: "ask" as const,
      discord_rich_presence: true,
      jvm_presets: [],
    };
    await settingsApi.update(settings);
    expect(invokeMock).toHaveBeenCalledWith("settings_update", { settings });
  });
});

describe("minecraftApi", () => {
  it("listVersions calls minecraft_list_versions", async () => {
    await minecraftApi.listVersions();
    expect(invokeMock).toHaveBeenCalledWith("minecraft_list_versions");
  });
});

describe("providersApi", () => {
  it("search maps provider/text to snake_case-free camelCase args", async () => {
    await providersApi.search("curseforge", "create");
    expect(invokeMock).toHaveBeenCalledWith("providers_search", { provider: "curseforge", text: "create", offset: 0 });
  });

  it("getVersions camelCases packId", async () => {
    await providersApi.getVersions("ftb", "42");
    expect(invokeMock).toHaveBeenCalledWith("providers_get_versions", { provider: "ftb", packId: "42" });
  });
});

describe("instancesApi", () => {
  it("get passes id", async () => {
    await instancesApi.get("abc");
    expect(invokeMock).toHaveBeenCalledWith("instances_get", { id: "abc" });
  });

  it("create camelCases every argument", async () => {
    await instancesApi.create("Demo", "1.20.1", "fabric", "0.16.0");
    expect(invokeMock).toHaveBeenCalledWith("instances_create", {
      name: "Demo",
      minecraftVersion: "1.20.1",
      loader: "fabric",
      loaderVersion: "0.16.0",
    });
  });

  it("rename passes id and name", async () => {
    await instancesApi.rename("abc", "New name");
    expect(invokeMock).toHaveBeenCalledWith("instances_rename", { id: "abc", name: "New name" });
  });

  it("updateSettings sends the settings object as-is", async () => {
    const settings = {
      min_memory_mb: 1024,
      max_memory_mb: 4096,
      extra_jvm_args: ["-Dfoo=bar"],
      java_path: null,
      window_width: 1280,
      window_height: 720,
      fullscreen: false,
      auto_join_server: "play.example.net",
    };
    await instancesApi.updateSettings("abc", settings);
    expect(invokeMock).toHaveBeenCalledWith("instances_update_settings", { id: "abc", settings });
  });

  it("openFolder passes an optional sub-folder", async () => {
    await instancesApi.openFolder("abc");
    expect(invokeMock).toHaveBeenCalledWith("instances_open_folder", { id: "abc", sub: null });
    await instancesApi.openFolder("abc", "mods");
    expect(invokeMock).toHaveBeenCalledWith("instances_open_folder", { id: "abc", sub: "mods" });
  });

  it("installModpack camelCases every argument", async () => {
    await instancesApi.installModpack("ftb", "1", "2", "Pack", "https://icon", "My Instance");
    expect(invokeMock).toHaveBeenCalledWith("instances_install_modpack", {
      provider: "ftb",
      packId: "1",
      versionId: "2",
      packName: "Pack",
      packIconUrl: "https://icon",
      instanceName: "My Instance",
    });
  });
});

describe("launchApi", () => {
  it("launch/stop/isRunning all camelCase instanceId", async () => {
    await launchApi.launch("abc");
    expect(invokeMock).toHaveBeenCalledWith("launch_instance", { instanceId: "abc", server: null });

    await launchApi.stop("abc");
    expect(invokeMock).toHaveBeenCalledWith("stop_instance", { instanceId: "abc" });

    await launchApi.isRunning("abc");
    expect(invokeMock).toHaveBeenCalledWith("is_instance_running", { instanceId: "abc" });
  });

  it("stats camelCases instanceId", async () => {
    await launchApi.stats("abc");
    expect(invokeMock).toHaveBeenCalledWith("instance_process_stats", { instanceId: "abc" });
  });
});
