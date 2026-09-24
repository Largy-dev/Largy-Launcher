/**
 * Markdown release notes flattened to readable plain text: headings, emphasis,
 * links and images lose their syntax, list markers become bullets. The result
 * is rendered as text, never as markup.
 */
export function changelogText(markdown: string): string {
  return markdown
    .replace(/\r\n/g, "\n")
    .replace(/!\[[^\]]*\]\([^)]*\)/g, "")
    .replace(/\[([^\]]+)\]\([^)]*\)/g, "$1")
    .replace(/^\s{0,3}#{1,6}\s+/gm, "")
    .replace(/^(\s*)[-*+]\s+/gm, "$1• ")
    .replace(/(\*\*|__)(.+?)\1/g, "$2")
    .replace(/`([^`]+)`/g, "$1")
    .replace(/^\s*(-{3,}|\*{3,})\s*$/gm, "")
    .replace(/\n{3,}/g, "\n\n")
    .trim();
}
