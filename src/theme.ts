export type AppTheme = "dark" | "light";

export const THEME_STORAGE_KEY = "mdv.theme";

export function resolveInitialTheme(
  storedTheme: string | null | undefined,
  prefersDark: boolean
): AppTheme {
  if (storedTheme === "dark" || storedTheme === "light") {
    return storedTheme;
  }

  return prefersDark ? "dark" : "light";
}

export function getNextTheme(theme: AppTheme): AppTheme {
  return theme === "dark" ? "light" : "dark";
}

export function getThemeToggleLabel(theme: AppTheme): string {
  return theme === "dark" ? "Switch to light theme" : "Switch to dark theme";
}

export function getMermaidTheme(theme: AppTheme): "dark" | "default" {
  return theme === "dark" ? "dark" : "default";
}
