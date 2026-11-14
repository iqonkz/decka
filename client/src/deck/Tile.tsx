import { useRef } from "react";
import type { LayoutItem } from "@decka/shared";
import { setPage } from "../app/controller";
import { getState } from "../store/store";

const TAP_SLOP_PX = 20;

/** Page index for a navigate target, or -1. */
function pageIndexOf(pageId: string): number {
  return getState().state?.layout.pages.findIndex((p) => p.id === pageId) ?? -1;
}

function ButtonTile({ item }: { item: Extract<LayoutItem, { kind: "button" }> }) {
  const down = useRef<{ x: number; y: number } | null>(null);
  return (
    <div
      className="tile-body"
      onPointerDown={(e) => {
        down.current = { x: e.clientX, y: e.clientY };
      }}
      onPointerUp={(e) => {
        const start = down.current;
        down.current = null;
        // A finger that moved far is a swipe, not a tap.
        if (!start || Math.hypot(e.clientX - start.x, e.clientY - start.y) > TAP_SLOP_PX) return;
        if (item.navigate !== undefined) {
          const index = pageIndexOf(item.navigate);
          if (index >= 0) setPage(index);
        }
      }}
    >
      {item.label ?? item.id}
    </div>
  );
}

/** Common tile container; the content depends on `kind`. */
export default function Tile({ item }: { item: LayoutItem }) {
  // `kind` comes from the PC and may be newer than this client: unknown kinds render as "?".
  const kind: string = item.kind;
  let content;
  switch (item.kind) {
    case "button":
      content = <ButtonTile item={item} />;
      break;
    case "clock":
    case "sys":
    case "statuses":
    case "status":
    case "notifications":
      content = <div className="tile-body">{kind}</div>; // widgets arrive in M5
      break;
    default:
      content = <div className="tile-body">?</div>;
  }
  return (
    <div
      className="tile"
      style={{ gridColumn: `${item.x + 1} / span ${item.w}`, gridRow: `${item.y + 1} / span ${item.h}` }}
    >
      {content}
    </div>
  );
}
