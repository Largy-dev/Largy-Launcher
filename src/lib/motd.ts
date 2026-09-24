/** Minecraft's 16 chat colours, by their `§` code. */
const COLORS: Record<string, string> = {
  "0": "#000000",
  "1": "#0000aa",
  "2": "#00aa00",
  "3": "#00aaaa",
  "4": "#aa0000",
  "5": "#aa00aa",
  "6": "#ffaa00",
  "7": "#aaaaaa",
  "8": "#555555",
  "9": "#5555ff",
  a: "#55ff55",
  b: "#55ffff",
  c: "#ff5555",
  d: "#ff55ff",
  e: "#ffff55",
  f: "#ffffff",
};

export interface MotdSegment {
  text: string;
  color: string | null;
  bold: boolean;
  italic: boolean;
  underline: boolean;
  strike: boolean;
}

const plain = (): Omit<MotdSegment, "text"> => ({
  color: null,
  bold: false,
  italic: false,
  underline: false,
  strike: false,
});

/**
 * Splits `§`-coded text (as the backend normalises server descriptions,
 * including `§#rrggbb` hex colours) into styled runs. Obfuscated text (`§k`)
 * is shown as-is.
 */
export function parseMotd(text: string): MotdSegment[] {
  const segments: MotdSegment[] = [];
  let style = plain();
  let buffer = "";
  const flush = () => {
    if (buffer) segments.push({ text: buffer, ...style });
    buffer = "";
  };

  for (let i = 0; i < text.length; i++) {
    if (text[i] !== "§" || i + 1 >= text.length) {
      buffer += text[i];
      continue;
    }
    const hex = /^#[0-9a-fA-F]{6}/.exec(text.slice(i + 1, i + 8));
    flush();
    if (hex) {
      style = { ...plain(), color: hex[0].toLowerCase() };
      i += 7;
      continue;
    }
    const code = text[i + 1].toLowerCase();
    i += 1;
    if (code in COLORS) style = { ...plain(), color: COLORS[code] };
    else if (code === "r") style = plain();
    else if (code === "l") style = { ...style, bold: true };
    else if (code === "o") style = { ...style, italic: true };
    else if (code === "n") style = { ...style, underline: true };
    else if (code === "m") style = { ...style, strike: true };
  }
  flush();
  return segments;
}

/**
 * The description split into lines, each trimmed: servers pad lines with
 * spaces to centre them in the game's wide list, which only wastes room here.
 */
export function parseMotdLines(text: string): MotdSegment[][] {
  const lines: MotdSegment[][] = [[]];
  for (const segment of parseMotd(text)) {
    segment.text.split("\n").forEach((part, i) => {
      if (i > 0) lines.push([]);
      if (part) lines[lines.length - 1].push({ ...segment, text: part });
    });
  }
  return lines
    .map((line) => {
      const out = line.map((s) => ({ ...s }));
      while (out.length && !(out[0].text = out[0].text.trimStart())) out.shift();
      while (out.length && !(out[out.length - 1].text = out[out.length - 1].text.trimEnd())) out.pop();
      return out;
    })
    .filter((line) => line.length > 0);
}

/** The description without any formatting codes. */
export function motdPlainText(text: string): string {
  return parseMotd(text)
    .map((s) => s.text)
    .join("");
}
