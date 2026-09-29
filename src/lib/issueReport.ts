export const REPO_URL = "https://github.com/Largy-dev/Largy-Launcher";

export interface IssueContext {
  appVersion: string;
  os: string;
  description: string;
  instance?: {
    name: string;
    minecraftVersion: string;
    loader: string;
    loaderVersion: string | null;
    mods: number | null;
  };
  crashSummary?: string | null;
  logUrls?: string[];
}

/** GitHub caps how long a prefilled URL can be; descriptions are trimmed to fit. */
const MAX_DESCRIPTION = 3000;

/** A prefilled "new issue" link: the player's words, then everything a maintainer needs. */
export function buildIssueUrl(ctx: IssueContext): string {
  const description = ctx.description.trim().slice(0, MAX_DESCRIPTION) || "_(décris ce qui s'est passé)_";
  const lines = [
    "### Ce qui se passe",
    description,
    "",
    "### Environnement",
    `- Largy Launcher ${ctx.appVersion}`,
    `- ${ctx.os}`,
  ];
  if (ctx.instance) {
    const i = ctx.instance;
    const loader = i.loader === "vanilla" ? "Vanilla" : `${i.loader} ${i.loaderVersion ?? ""}`.trim();
    lines.push(`- Instance : Minecraft ${i.minecraftVersion}, ${loader}${i.mods !== null ? `, ${i.mods} mods` : ""}`);
  }
  if (ctx.crashSummary) lines.push("", "### Diagnostic du launcher", ctx.crashSummary);
  if (ctx.logUrls?.length) lines.push("", "### Logs", ...ctx.logUrls.map((u) => `- ${u}`));

  const firstLine = ctx.description.trim().split("\n")[0].slice(0, 80);
  const title = firstLine || (ctx.crashSummary ? `Crash : ${ctx.crashSummary.slice(0, 70)}` : "Problème");
  const params = new URLSearchParams({ title, body: lines.join("\n"), labels: "bug" });
  return `${REPO_URL}/issues/new?${params.toString()}`;
}
