import { argbFromHex, hexFromArgb, Hct, MaterialDynamicColors, SchemeTonalSpot } from "@material/material-color-utilities";
import { DEFAULT_ACCENT, parseHex } from "./colors";

/** The roles the Material theme uses (DESIGN.md 1.3), as `#rrggbb`. */
export interface MaterialRoles {
  surface: string;
  surfaceContainer: string;
  surfaceContainerHigh: string;
  onSurface: string;
  onSurfaceVariant: string;
  primary: string;
  primaryContainer: string;
  onPrimaryContainer: string;
  outlineVariant: string;
}

const cache = new Map<string, MaterialRoles>();

/** Dark tonal-spot scheme (contrast 0) generated from `seed`; cached per colour. */
export function materialRoles(seed: string): MaterialRoles {
  const key = (parseHex(seed) ? seed : DEFAULT_ACCENT).toLowerCase();
  const hit = cache.get(key);
  if (hit) return hit;
  const scheme = new SchemeTonalSpot(Hct.fromInt(argbFromHex(key)), true, 0);
  const role = (c: { getArgb: (s: SchemeTonalSpot) => number }): string => hexFromArgb(c.getArgb(scheme));
  const roles: MaterialRoles = {
    surface: role(MaterialDynamicColors.surface),
    surfaceContainer: role(MaterialDynamicColors.surfaceContainer),
    surfaceContainerHigh: role(MaterialDynamicColors.surfaceContainerHigh),
    onSurface: role(MaterialDynamicColors.onSurface),
    onSurfaceVariant: role(MaterialDynamicColors.onSurfaceVariant),
    primary: role(MaterialDynamicColors.primary),
    primaryContainer: role(MaterialDynamicColors.primaryContainer),
    onPrimaryContainer: role(MaterialDynamicColors.onPrimaryContainer),
    outlineVariant: role(MaterialDynamicColors.outlineVariant),
  };
  cache.set(key, roles);
  return roles;
}

/** CSS variables of the whole screen: the scheme of the accent colour. */
export function materialRootVars(accent: string): Record<string, string> {
  const r = materialRoles(accent);
  return {
    "--m-surface": r.surface,
    "--m-surface-container": r.surfaceContainer,
    "--m-surface-container-high": r.surfaceContainerHigh,
    "--m-on-surface": r.onSurface,
    "--m-on-surface-variant": r.onSurfaceVariant,
    "--m-primary": r.primary,
    "--m-primary-container": r.primaryContainer,
    "--m-on-primary-container": r.onPrimaryContainer,
    "--m-outline-variant": r.outlineVariant,
  };
}

/** CSS variables of a coloured key: its own scheme (seed = the key colour). */
export function materialKeyVars(color: string): Record<string, string> {
  const r = materialRoles(color);
  return { "--mk-container": r.primaryContainer, "--mk-on": r.onPrimaryContainer };
}
