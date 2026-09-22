/** Splits a space-separated JVM args string into an array, dropping empty tokens. */
export function parseJvmArgs(value: string): string[] {
  return value.trim().split(/\s+/).filter(Boolean);
}
