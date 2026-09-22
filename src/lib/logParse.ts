export type LogLevel = "debug" | "info" | "warn" | "error";

export interface RawLogLine {
  line: string;
  stream: "stdout" | "stderr";
}

export interface ParsedLogLine {
  raw: string;
  level: LogLevel;
  time: string | null;
  source: string | null;
  message: string;
  /** Part of a stack trace (`at …`, `Caused by: …`, `... 12 more`). */
  stack: boolean;
}

// [12:34:56] [Render thread/INFO]: msg
// [22Sep2026 12:34:56.123] [main/WARN] [mixin/]: msg
const HEADER_RE = /^\[([^\]]*?\d{2}:\d{2}:\d{2}[^\]]*)\]\s*\[([^\]/]+)\/([A-Z]+)\](?:\s*\[([^\]]*)\])?:?\s?(.*)$/;
const STACK_RE = /^\s*(at\s+\S|\.\.\.\s*\d+\s+more|Caused by:|Suppressed:)/;
const EXCEPTION_RE = /^(Exception in thread|[\w$.]+(Exception|Error)(:|$))/;

const LEVELS: Record<string, LogLevel> = {
  TRACE: "debug",
  DEBUG: "debug",
  INFO: "info",
  WARN: "warn",
  WARNING: "warn",
  ERROR: "error",
  FATAL: "error",
  SEVERE: "error",
};

/**
 * Parses one launch's log lines. Lines without their own header (stack
 * traces, multi-line messages) inherit the previous line's level so a crash
 * trace stays red all the way down.
 */
export function parseLogs(lines: RawLogLine[]): ParsedLogLine[] {
  let previous: LogLevel = "info";
  return lines.map(({ line, stream }) => {
    const header = HEADER_RE.exec(line);
    if (header) {
      const level = LEVELS[header[3]] ?? "info";
      previous = level;
      const source = header[4] ? `${header[2]} · ${header[4]}` : header[2];
      return { raw: line, level, time: header[1], source, message: header[5], stack: false };
    }
    const stack = STACK_RE.test(line);
    let level = previous;
    if (EXCEPTION_RE.test(line.trim())) level = "error";
    else if (!stack && stream === "stderr" && previous === "info") level = "warn";
    if (stack || level === "error") previous = level;
    return { raw: line, level, time: null, source: null, message: line, stack };
  });
}

export function countByLevel(lines: ParsedLogLine[]): Record<LogLevel, number> {
  const counts: Record<LogLevel, number> = { debug: 0, info: 0, warn: 0, error: 0 };
  for (const line of lines) counts[line.level]++;
  return counts;
}
