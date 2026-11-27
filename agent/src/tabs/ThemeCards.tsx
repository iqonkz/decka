import { useEffect, useMemo, useRef, useState } from "react";
import { t, type DisplaySettings, type Page, type ThemeId } from "@decka/shared";
import { EMPTY_DATA, GlassLayer, Grid, resolveTheme } from "@decka/ui";
import PhoneFrame from "./PhoneFrame";
import PhIcon from "../ui/PhIcon";

/** The names of the four looks are product names and are not translated. */
const THEME_NAMES: Record<ThemeId, string> = { amoled: "AMOLED", studio: "Studio", material: "Material", glass: "Glass" };
const themeHint = (id: ThemeId): string => t(`theme.hint_${id}`);
export const THEME_IDS: ThemeId[] = ["amoled", "studio", "material", "glass"];

/** Six sample keys in a 3x2 grid, enough to show how a theme looks (labels in the language of the window). */
const sample = (): Page => ({
  id: "sample",
  name: t("theme.sample"),
  items: [
    { id: "s1", kind: "button", x: 0, y: 0, w: 1, h: 1, label: t("theme.s_play"), icon: "lucide:play", actionId: "a" },
    { id: "s2", kind: "button", x: 1, y: 0, w: 1, h: 1, label: t("theme.s_mute"), icon: "lucide:volume-x", actionId: "a", color: "#b45309" },
    { id: "s3", kind: "button", x: 2, y: 0, w: 1, h: 1, label: t("theme.s_code"), icon: "lucide:code", actionId: "a" },
    { id: "s4", kind: "button", x: 0, y: 1, w: 1, h: 1, label: t("theme.s_mic"), icon: "lucide:mic", actionId: "a", color: "#16a34a" },
    { id: "s5", kind: "button", x: 1, y: 1, w: 1, h: 1, label: t("theme.s_home"), icon: "lucide:home", actionId: "a" },
    { id: "s6", kind: "button", x: 2, y: 1, w: 1, h: 1, label: t("theme.s_lock"), icon: "lucide:lock", actionId: "a", color: "#b91c1c" },
  ],
});
const DATA = { ...EMPTY_DATA, online: true, preview: true };
const NOOP = { onPress: () => Promise.resolve(), onPressError: () => {}, onNavigate: () => {}, onDismiss: () => {} };

function Preview({ theme, display }: { theme: ThemeId; display: DisplaySettings }) {
  const shown = useMemo(() => ({ ...display, theme }), [display, theme]);
  const resolved = resolveTheme(shown);
  // The frame follows the orientation of the deck; the six sample keys turn with it (3x2 -> 2x3).
  const portrait = display.orientation === "portrait";
  const page = sample();
  if (portrait) page.items = page.items.map((i) => ({ ...i, x: i.y, y: i.x }));
  const box = useRef<HTMLSpanElement>(null);
  const [width, setWidth] = useState(210);
  useEffect(() => {
    const el = box.current;
    if (!el) return;
    // Less the 1 px on each side, where the outline of the frame is drawn.
    const measure = () => {
      if (el.clientWidth > 2) setWidth(el.clientWidth - 2);
    };
    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(el);
    return () => observer.disconnect();
  }, []);
  return (
    <span ref={box} className="theme-card-preview">
      <PhoneFrame availableWidth={width} {...(portrait ? { maxHeight: 190 } : {})} display={shown}>
        {theme === "glass" && <GlassLayer wallpaper={resolved.wallpaper} art={undefined} />}
        <div style={{ position: "absolute", inset: 0 }}>
          <Grid page={page} grid={portrait ? { cols: 2, rows: 3 } : { cols: 3, rows: 2 }} data={DATA} {...NOOP} />
        </div>
      </PhoneFrame>
    </span>
  );
}

/** The four looks as cards with a live preview; `display` supplies the accent and wallpaper the preview uses. */
export default function ThemeCards({ value, display, onPick }: { value: ThemeId; display: DisplaySettings; onPick: (theme: ThemeId) => void }) {
  return (
    <div className="theme-cards" role="radiogroup" aria-label={t("tab.look")}>
      {THEME_IDS.map((id) => {
        const isSelected = value === id;
        return (
          <button
            key={id}
            type="button"
            role="radio"
            aria-checked={isSelected}
            className={isSelected ? "theme-card selected" : "theme-card"}
            onClick={() => onPick(id)}
          >
            {isSelected && (
              <span className="selection-badge">
                <PhIcon name="check-circle" size={20} weight="fill" color="var(--accent)" />
              </span>
            )}
            <Preview theme={id} display={display} />
            <strong>{THEME_NAMES[id]}</strong>
            <small className="muted">{themeHint(id)}</small>
          </button>
        );
      })}
    </div>
  );
}
