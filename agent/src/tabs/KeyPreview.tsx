import { useMemo } from "react";
import type { ButtonItem, DisplaySettings, Page } from "@decka/shared";
import { EMPTY_DATA, GlassLayer, Grid, resolveTheme } from "@decka/ui";
import PhoneFrame from "./PhoneFrame";
import { useIconPreviews } from "./useIconPreviews";

const noop = { onPress: () => Promise.resolve(), onPressError: () => {}, onNavigate: () => {}, onDismiss: () => {} };
/** A 96 px key (the screen is a square with the key's margins around it, as on the phone). */
const SIDE = 96;

/** The key as it looks on the phone, in the theme of the deck, large, at the top of the properties panel (DESIGN.md 4.6). */
export default function KeyPreview({ item, display }: { item: ButtonItem; display: DisplaySettings }) {
  const page: Page = useMemo(() => ({ id: "preview", name: "", items: [{ ...item, x: 0, y: 0, w: 1, h: 1 }] }), [item]);
  const icons = useIconPreviews(page);
  const { keyFace, keyLabels } = display;
  const data = useMemo(() => ({ ...EMPTY_DATA, online: true, icons, ...(keyFace !== undefined ? { keyFace } : {}), ...(keyLabels !== undefined ? { keyLabels } : {}) }), [icons, keyFace, keyLabels]);
  const theme = resolveTheme(display);
  const view = Math.round(SIDE * 1.25);
  return (
    <div className="key-preview" aria-label="Key preview">
      <PhoneFrame availableWidth={view} display={display} view={{ w: view, h: view }}>
        {theme.theme === "glass" && <GlassLayer wallpaper={theme.wallpaper} art={undefined} />}
        <div style={{ position: "absolute", inset: 0 }}>
          <Grid page={page} grid={{ cols: 1, rows: 1 }} data={data} {...noop} />
        </div>
      </PhoneFrame>
    </div>
  );
}