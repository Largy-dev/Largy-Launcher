import { beforeEach, describe, expect, it } from "vitest";

import { applyAccent, loadAccent } from "./theme";

describe("loadAccent / applyAccent", () => {
  beforeEach(() => {
    localStorage.clear();
    document.documentElement.removeAttribute("data-accent");
  });

  it("defaults to green when nothing is stored", () => {
    expect(loadAccent()).toBe("green");
  });

  it("returns the previously applied accent", () => {
    applyAccent("violet");
    expect(loadAccent()).toBe("violet");
  });

  it("falls back to the default for a corrupted/unknown stored value", () => {
    localStorage.setItem("largy-accent", "not-a-real-accent");
    expect(loadAccent()).toBe("green");
  });

  it("sets the data-accent attribute on the document root", () => {
    applyAccent("blue");
    expect(document.documentElement.getAttribute("data-accent")).toBe("blue");
  });
});
