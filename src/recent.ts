export function normalizeRecentPath(path: string): string {
  const windowsExtendedUncPrefix = "\\\\?\\UNC\\";
  const windowsExtendedPrefix = "\\\\?\\";
  const lowerPath = path.toLowerCase();

  if (lowerPath.startsWith(windowsExtendedUncPrefix.toLowerCase())) {
    return `\\\\${path.slice(windowsExtendedUncPrefix.length)}`;
  }

  if (lowerPath.startsWith(windowsExtendedPrefix.toLowerCase())) {
    return path.slice(windowsExtendedPrefix.length);
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
