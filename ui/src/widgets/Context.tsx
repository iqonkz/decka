import { useMemo } from "react";
import { t, type ButtonItem, type ContextItem } from "@decka/shared";
import { useDeckEnv } from "../deckEnv";
import { useDeckMetrics } from "../metricsContext";
import Tile from "../Tile";

/** The keys a context page offers inside a widget: plain keys in reading order (a key that only navigates is left out). */
export function contextKeys(items: readonly { kind: string }[]): ButtonItem[] {
  return (items.filter((i) => i.kind === "button") as ButtonItem[])
    .filter((b) => b.navigate === undefined)
    .sort((a, b) => a.y - b.y || a.x - b.x);
}

/** The page whose `apps` has the app in front, or undefined. */
export function pageForApp<P extends { apps?: string[] }>(pages: readonly P[] | undefined, app: string | null | undefined): P | undefined {
  if (!app || !pages) return undefined;
  return pages.find((p) => p.apps?.includes(app) === true);
}

/**
 * The context widget: while an app that has a page is in front on the PC, the keys of that page fill the widget's cells
 * (as many as fit), so the page does not have to be swiped to. With no page for the app it says so.
 */
export default function Context({ item }: { item: ContextItem }) {
  const { pages, focusApp } = useDeckEnv();
  const { side, gap, cellW, cellH } = useDeckMetrics();
  const page = pageForApp(pages, focusApp);
  const keys = useMemo(() => (page ? contextKeys(page.items).slice(0, item.w * item.h) : []), [page, item.w, item.h]);

  if (!page || keys.length === 0) {
    return (
      <div className="context context-empty">
        <span>{t("context.none")}</span>
        {focusApp && <small>{focusApp}</small>}
      </div>
    );
  }
  // The keys have the cells of the main grid, so the widget is a small grid of them.
  return (
    <div
      className="context"
      style={{ display: "grid", gridTemplateColumns: `repeat(${item.w}, ${cellW ?? side}px)`, gridTemplateRows: `repeat(${item.h}, ${cellH ?? side}px)`, gap }}
      aria-label={page.name}
    >
      {keys.map((k, i) => (
        <Tile key={k.id} item={{ ...k, x: i % item.w, y: Math.floor(i / item.w), w: 1, h: 1 }} />
      ))}
    </div>
  );
}
