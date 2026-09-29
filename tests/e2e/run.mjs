// End-to-end smoke test of the real app window, driven over the WebView2
// DevTools protocol.
//
//   npx tauri build --debug --no-bundle --config tests/e2e/tauri.e2e.json
//   node tests/e2e/run.mjs [path/to/largy-launcher.exe]
//
// It creates (then deletes) an "E2E" instance in the app's data folder. On a
// throwaway machine (CI), LARGY_E2E_SEED=1 first writes settings for offline
// play so no Microsoft account is needed. Close any other Largy Launcher
// first: the app is single-instance.

import { spawn } from "node:child_process";
import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";

import { chromium } from "playwright-core";

const exe = process.argv[2] ?? "src-tauri/target/debug/largy-launcher.exe";
const artifacts = "tests/e2e/artifacts";
const CDP = "http://127.0.0.1:9222";

if (process.env.LARGY_E2E_SEED === "1") {
  const dir = join(process.env.APPDATA, "com.largylauncher.app");
  mkdirSync(dir, { recursive: true });
  writeFileSync(
    join(dir, "settings.json"),
    JSON.stringify({
      default_min_memory_mb: 1024,
      default_max_memory_mb: 2048,
      default_jvm_args: [],
      offline_mode: true,
      offline_username: "E2E",
    }),
  );
}

const app = spawn(exe, [], { stdio: "ignore" });
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

async function waitForCdp() {
  for (let i = 0; i < 90; i++) {
    try {
      const res = await fetch(`${CDP}/json/version`);
      if (res.ok) return;
    } catch {
      // not up yet
    }
    await sleep(1000);
  }
  throw new Error("the app never exposed its DevTools port");
}

const failures = [];
const pageErrors = [];
let page;

async function step(name, fn) {
  try {
    await fn();
    console.log(`  ✓ ${name}`);
  } catch (e) {
    failures.push(name);
    console.log(`  ✗ ${name}\n    ${String(e?.message ?? e).split("\n")[0]}`);
    mkdirSync(artifacts, { recursive: true });
    await page?.screenshot({ path: `${artifacts}/${name.replace(/\W+/g, "-")}.png` }).catch(() => {});
  }
}

const invoke = (cmd, args) => page.evaluate(([c, a]) => window.__TAURI_INTERNALS__.invoke(c, a), [cmd, args]);
const go = (hash) => page.evaluate((h) => (location.hash = h), hash);
const text = (t, timeout = 15_000) => page.getByText(t, { exact: false }).first().waitFor({ timeout });

let instanceId = null;
let browser;
try {
  await waitForCdp();
  browser = await chromium.connectOverCDP(CDP);
  const context = browser.contexts()[0];
  for (let i = 0; i < 30 && !page; i++) {
    page = context.pages().find((p) => p.url().includes("tauri.localhost"));
    if (!page) await sleep(500);
  }
  if (!page) throw new Error("app page not found");
  page.on("pageerror", (e) => pageErrors.push(e.message));
  page.on("console", (m) => {
    if (m.type() === "error" && !/Failed to load resource/.test(m.text())) pageErrors.push(m.text());
  });

  console.log("Largy Launcher E2E");
  await step("home renders", async () => {
    await page.waitForFunction(() => /Mes instances|Prêt pour l'aventure/.test(document.body.innerText), null, {
      timeout: 30_000,
    });
  });

  await step("an instance can be created and listed", async () => {
    const instance = await invoke("instances_create", {
      name: "E2E Vanilla",
      minecraftVersion: "1.21.1",
      loader: "vanilla",
      loaderVersion: null,
    });
    instanceId = instance.id;
    await page.reload();
    await text("E2E Vanilla");
  });

  await step("content screen shows its tabs", async () => {
    await go(`#/instances/${instanceId}/content`);
    await text("Resource packs");
    await page.getByRole("tab", { name: /Mondes/ }).click();
    await text("Aucun monde");
  });

  await step("the catalogue lists compatible resource packs", async () => {
    await go(`#/instances/${instanceId}/content?tab=resource_pack`);
    await page.getByRole("button", { name: "Parcourir", exact: true }).click();
    await text("Ajouter du contenu");
    await page.getByRole("button", { name: "Installer" }).first().waitFor({ timeout: 30_000 });
    await page.keyboard.press("Escape");
  });

  await step("a restore point can be created", async () => {
    await go(`#/instances/${instanceId}`);
    const create = page.getByRole("button", { name: "Créer maintenant" });
    await create.scrollIntoViewIfNeeded();
    await create.click();
    await text("Point de restauration manuel");
  });

  await step("the modpack browser loads packs", async () => {
    await go("#/modpacks");
    await page.locator('button[aria-label*=", par"]').first().waitFor({ timeout: 30_000 });
  });

  await step("settings render", async () => {
    await go("#/settings?tab=game");
    await text("Mêmes options dans toutes les instances");
  });

  await step("no errors in the page", async () => {
    if (pageErrors.length) throw new Error(pageErrors.join(" | "));
  });
} catch (e) {
  failures.push(`setup: ${e.message}`);
  console.log(`  ✗ ${e.message}`);
} finally {
  if (instanceId && page) {
    await invoke("instances_delete", { id: instanceId }).catch(() => {});
  }
  await browser?.close().catch(() => {});
  app.kill();
}

if (failures.length) {
  console.log(`\n${failures.length} échec(s)`);
  process.exit(1);
}
console.log("\nTout est bon.");
process.exit(0);
