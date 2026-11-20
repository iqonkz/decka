import { useEffect, useSyncExternalStore } from "react";
import { getLanguage, isRtl, resolveLanguage, setLanguage, subscribeLanguage, type Language, type LanguageSetting } from "@decka/shared";

/** The current interface language; the component re-renders when it changes (the root of an app uses this). */
export function useLanguage(): Language {
  return useSyncExternalStore(subscribeLanguage, getLanguage, getLanguage);
}

/**
 * Applies `display.language` (`auto` follows `systemLanguage`) to `t()` and to `<html lang>`. Called once at the root
 * of an app with the setting that came with the layout; without a layout it is `auto`.
 */
export function useApplyLanguage(setting: LanguageSetting | undefined, systemLanguage: string | undefined): Language {
  const language = resolveLanguage(setting, systemLanguage);
  // Layout effect timing is not needed: the subscribers re-render on the change.
  useEffect(() => {
    setLanguage(language);
    document.documentElement.lang = language;
    // Arabic is written from right to left: the window and the menus mirror; the grid and the pages keep `dir="ltr"` themselves.
    document.documentElement.dir = isRtl(language) ? "rtl" : "ltr";
  }, [language]);
  return useLanguage();
}
