export function normalizeRecentPath(path: string): string {
  if (path.startsWith("\\\\?\\UNC\\")) {
    return `\\\\${path.slice("\\\\?\\UNC\\".length)}`;
  }

  if (path.startsWith("\\\\?\\")) {
    return path.slice("\\\\?\\".length);
  }

  return path;
}

export function recentFileName(path: string): string {
  const normalizedPath = normalizeRecentPath(path);
  const segments = normalizedPath.split(/[\\/]/).filter(Boolean);
  return segments.at(-1) ?? normalizedPath;
}

export function recentMenuLabel(path: string, index: number): string {
  return `${index + 1}. ${recentFileName(path)}`;
}
