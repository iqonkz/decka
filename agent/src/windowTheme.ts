import { useEffect, useState } from "react";
import type { WindowTheme } from "@decka/shared";

export const WINDOW_THEME_STORAGE_KEY = "decka.windowTheme";
export const WINDOW_THEME_EVENT = "decka:window-theme";

export function getSavedWindowTheme(): WindowTheme {
  try {
    const val = localStorage.getItem(WINDOW_THEME_STORAGE_KEY);
    if (val === "light" || val === "dark" || val === "system") return val;
  } catch {}
  return "system";
}

export function applyWindowTheme(theme: WindowTheme) {
  try {
    localStorage.setItem(WINDOW_THEME_STORAGE_KEY, theme);
  } catch {}
  document.documentElement.setAttribute("data-window-theme", theme);
  const darkMql = window.matchMedia("(prefers-color-scheme: dark)");
  const isDark = theme === "dark" || (theme === "system" && darkMql.matches);
  document.documentElement.style.colorScheme = isDark ? "dark" : "light";
  window.dispatchEvent(new CustomEvent(WINDOW_THEME_EVENT, { detail: { theme } }));
}

/** Hook to synchronize the document window theme with the layout/config setting. */
export function useApplyWindowTheme(setting?: WindowTheme) {
  const [theme, setTheme] = useState<WindowTheme>(() => setting ?? getSavedWindowTheme());

  useEffect(() => {
    if (setting) setTheme(setting);
  }, [setting]);

  useEffect(() => {
    applyWindowTheme(theme);
    if (theme !== "system") return;
    const mql = window.matchMedia("(prefers-color-scheme: dark)");
    const onChange = (e: MediaQueryListEvent) => {
      document.documentElement.style.colorScheme = e.matches ? "dark" : "light";
    };
    mql.addEventListener("change", onChange);
    return () => mql.removeEventListener("change", onChange);
  }, [theme]);
}
