import { beforeEach, describe, expect, it } from "vitest";

import { applyTheme, isAccent, loadLegacyAccent, readableForeground, type ThemeSettings } from "./theme";

const base: ThemeSettings = {
  themeMode: "dark",
  accent: "green",
  customAccent: "#22c55e",
  uiScale: 100,
  animations: "full",
};

describe("applyTheme", () => {
  beforeEach(() => {
    const root = document.documentElement;
    root.className = "";
    root.removeAttribute("data-accent");
    root.removeAttribute("style");
  });

  it("toggles the dark class from the mode", () => {
    applyTheme(base);
    expect(document.documentElement.classList.contains("dark")).toBe(true);
    applyTheme({ ...base, themeMode: "light" });
    expect(document.documentElement.classList.contains("dark")).toBe(false);
  });

  it("sets the data-accent attribute on the document root", () => {
    applyTheme({ ...base, accent: "blue" });
    expect(document.documentElement.getAttribute("data-accent")).toBe("blue");
  });

  it("exposes a custom accent and its readable foreground as CSS variables", () => {
    applyTheme({ ...base, accent: "custom", customAccent: "#ffee00" });
    const style = document.documentElement.style;
    expect(document.documentElement.getAttribute("data-accent")).toBe("custom");
    expect(style.getPropertyValue("--accent-custom")).toBe("#ffee00");
    expect(style.getPropertyValue("--accent-custom-fg")).toBe("#0b0f19");
  });

  it("falls back to the default accent when the custom color is invalid", () => {
    applyTheme({ ...base, accent: "custom", customAccent: "nope" });
    expect(document.documentElement.getAttribute("data-accent")).toBe("green");
  });

  it("scales the root font size and exposes the motion level", () => {
    applyTheme({ ...base, uiScale: 110, animations: "none" });
    expect(document.documentElement.style.fontSize).toBe("110%");
    expect(document.documentElement.getAttribute("data-motion")).toBe("none");
  });
});

describe("readableForeground", () => {
  it("picks white on dark colors and near-black on light ones", () => {
    expect(readableForeground("#1e3a8a")).toBe("#ffffff");
    expect(readableForeground("#fde047")).toBe("#0b0f19");
  });
});

describe("loadLegacyAccent", () => {
  beforeEach(() => localStorage.clear());

  it("returns the accent saved by older versions", () => {
    localStorage.setItem("largy-accent", "violet");
    expect(loadLegacyAccent()).toBe("violet");
  });

  it("ignores missing or unknown values", () => {
    expect(loadLegacyAccent()).toBeNull();
    localStorage.setItem("largy-accent", "not-a-real-accent");
    expect(loadLegacyAccent()).toBeNull();
  });
});

describe("isAccent", () => {
  it("accepts presets and custom only", () => {
    expect(isAccent("orange")).toBe(true);
    expect(isAccent("custom")).toBe(true);
    expect(isAccent("teal")).toBe(false);
  });
});
