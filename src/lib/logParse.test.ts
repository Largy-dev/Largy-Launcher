import { describe, expect, it } from "vitest";

import { countByLevel, parseLogs, type RawLogLine } from "./logParse";

const out = (line: string): RawLogLine => ({ line, stream: "stdout" });

describe("parseLogs", () => {
  it("parses vanilla headers", () => {
    const [line] = parseLogs([out("[12:34:56] [Render thread/WARN]: Missing texture")]);
    expect(line).toMatchObject({
      level: "warn",
      time: "12:34:56",
      source: "Render thread",
      message: "Missing texture",
    });
  });

  it("parses Forge headers with a logger name", () => {
    const [line] = parseLogs([out("[22Sep2026 12:34:56.123] [main/ERROR] [mixin/]: Mixin apply failed")]);
    expect(line).toMatchObject({ level: "error", source: "main · mixin/", message: "Mixin apply failed" });
  });

  it("maps FATAL to error and keeps stack traces at the level of their header", () => {
    const parsed = parseLogs([
      out("[12:00:00] [main/FATAL]: Crash"),
      out("java.lang.NullPointerException: boom"),
      out("\tat net.minecraft.Foo.bar(Foo.java:1)"),
      out("Caused by: java.io.IOException"),
      out("[12:00:01] [main/INFO]: back to normal"),
    ]);
    expect(parsed.map((l) => l.level)).toEqual(["error", "error", "error", "error", "info"]);
    expect(parsed[2].stack).toBe(true);
  });

  it("treats headerless stderr lines as warnings", () => {
    const [line] = parseLogs([{ line: "WARNING: A restricted method was called", stream: "stderr" }]);
    expect(line.level).toBe("warn");
  });
});

describe("countByLevel", () => {
  it("counts each level", () => {
    const counts = countByLevel(
      parseLogs([out("[01:00:00] [a/INFO]: x"), out("[01:00:00] [a/WARN]: y"), out("[01:00:00] [a/ERROR]: z")]),
    );
    expect(counts.warn).toBe(1);
    expect(counts.error).toBe(1);
  });
});
