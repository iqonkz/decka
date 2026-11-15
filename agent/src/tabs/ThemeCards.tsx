import { useMemo } from "react";
import type { DisplaySettings, Page, ThemeId } from "@decka/shared";
import { EMPTY_DATA, GlassLayer, Grid, resolveTheme } from "@decka/ui";
import PhoneFrame from "./PhoneFrame";

const THEME_NAMES: Record<ThemeId, string> = { amoled: "AMOLED", studio: "Studio", material: "Material", glass: "Glass" };
const THEME_HINTS: Record<ThemeId, string> = {
  amoled: "Black screen, thin outlines",
  studio: "Hardware deck, depth and shadow",
  material: "Tonal colours from your accent",
  glass: "Frosted glass over a wallpaper",
};
export const THEME_IDS: ThemeId[] = ["amoled", "studio", "material", "glass"];

/** Six sample keys in a 3x2 grid, enough to show how a theme looks. */
const SAMPLE: Page = {
  id: "sample",
  name: "Sample",
  items: [
    { id: "s1", kind: "button", x: 0, y: 0, w: 1, h: 1, label: "Play", icon: "lucide:play", actionId: "a" },
    { id: "s2", kind: "button", x: 1, y: 0, w: 1, h: 1, label: "Mute", icon: "lucide:volume-x", actionId: "a", color: "#b45309" },
    { id: "s3", kind: "button", x: 2, y: 0, w: 1, h: 1, label: "Code", icon: "lucide:code", actionId: "a" },
    { id: "s4", kind: "button", x: 0, y: 1, w: 1, h: 1, label: "Mic", icon: "lucide:mic", actionId: "a", color: "#16a34a" },
    { id: "s5", kind: "button", x: 1, y: 1, w: 1, h: 1, label: "Home", icon: "lucide:home", actionId: "a" },
    { id: "s6", kind: "button", x: 2, y: 1, w: 1, h: 1, label: "Lock", icon: "lucide:lock", actionId: "a", color: "#b91c1c" },
  ],
};
const DATA = { ...EMPTY_DATA, online: true };
const NOOP = { onPress: () => Promise.resolve(), onPressError: () => {}, onNavigate: () => {}, onDismiss: () => {} };

function Preview({ theme, display }: { theme: ThemeId; display: DisplaySettings }) {
  const shown = useMemo(() => ({ ...display, theme }), [display, theme]);
  const resolved = resolveTheme(shown);
  return (
    <PhoneFrame availableWidth={210} display={shown}>
      {theme === "glass" && <GlassLayer wallpaper={resolved.wallpaper} art={undefined} />}
      <div style={{ position: "absolute", inset: 0 }}>
        <Grid page={SAMPLE} grid={{ cols: 3, rows: 2 }} data={DATA} {...NOOP} />
      </div>
    </PhoneFrame>
  );
}

/** The four looks as cards with a live preview; `display` supplies the accent and wallpaper the preview uses. */
export default function ThemeCards({ value, display, onPick }: { value: ThemeId; display: DisplaySettings; onPick: (theme: ThemeId) => void }) {
  return (
    <div className="theme-cards" role="radiogroup" aria-label="Look">
      {THEME_IDS.map((id) => (
        <button key={id} type="button" role="radio" aria-checked={value === id} className={value === id ? "theme-card selected" : "theme-card"} onClick={() => onPick(id)}>
          <Preview theme={id} display={display} />
          <strong>{THEME_NAMES[id]}</strong>
          <small className="muted">{THEME_HINTS[id]}</small>
        </button>
      ))}
    </div>
  );
}
