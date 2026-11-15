import type { DisplaySettings, ThemeId, WallpaperId } from "@decka/shared";
import { DEFAULT_ACCENT } from "./colors";
import { materialRootVars } from "./material";

export { DEFAULT_ACCENT };

export const THEMES: ThemeId[] = ["amoled", "studio", "material", "glass"];
export const WALLPAPERS: WallpaperId[] = ["aurora", "sunset", "ocean", "graphite", "album"];

export interface ResolvedTheme {
  theme: ThemeId;
  accent: string;
  wallpaper: WallpaperId;
}

/**
 * The look a layout asks for. A missing or unknown value falls back to the documented default:
 * `amoled` (existing users keep their black screen), accent `#7c5cff`, wallpaper `aurora`.
 */
export function resolveTheme(display: Pick<DisplaySettings, "theme" | "accent" | "wallpaper"> | null | undefined): ResolvedTheme {
  const theme = THEMES.find((t) => t === display?.theme) ?? "amoled";
  const accent = display?.accent && /^#[0-9a-fA-F]{6}$/.test(display.accent) ? display.accent : DEFAULT_ACCENT;
  const wallpaper = WALLPAPERS.find((w) => w === display?.wallpaper) ?? "aurora";
  return { theme, accent, wallpaper };
}

/** Puts the theme on `root`: `data-theme`, `data-wallpaper` and the `--accent` variable. */
export function applyTheme(
  root: HTMLElement,
  display: Pick<DisplaySettings, "theme" | "accent" | "wallpaper"> | null | undefined,
  options: { reduced?: boolean } = {},
): ResolvedTheme {
  const resolved = resolveTheme(display);
  root.dataset.theme = resolved.theme;
  root.dataset.wallpaper = resolved.wallpaper;
  // `reduced`: slow device or "Reduce effects" - the Glass theme drops its refraction layer.
  if (options.reduced) root.dataset.reduced = "1";
  else delete root.dataset.reduced;
  root.style.setProperty("--accent", resolved.accent);
  // The Material scheme comes from the accent; other themes do not need it.
  if (resolved.theme === "material") {
    for (const [name, value] of Object.entries(materialRootVars(resolved.accent))) root.style.setProperty(name, value);
  }
  return resolved;
}
