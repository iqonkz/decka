import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { Page } from "@decka/shared";
import { gridMetrics, PopupGrid, popupSize } from "@decka/ui";
import { closePopup } from "../store/store";
import { placePopup, POPUP_CLOSE_DELAY_MS, POPUP_IDLE_MS, POPUP_SWIPE_DOWN_PX, type Rect } from "./popup";
import { useDeckProps } from "./useDeckProps";

const CLOSE_ANIMATION_MS = 140;
const SCREEN_PAD_PX = 12;

interface Props {
  page: Page;
  anchor: Rect;
  grid: { cols: number; rows: number };
}

/**
 * A popup group over the current page (DESIGN.md 4.1): the keys of a popup page in a panel above the key that opened
 * it. It closes on a tap outside, 250 ms after a successful press inside (unless `stayOpen`), after 10 s without a
 * touch, and on a downward swipe.
 */
export default function PopupPanel({ page, anchor, grid }: Props) {
  const props = useDeckProps();
  const [closing, setClosing] = useState(false);
  const closingRef = useRef(false);
  const swipe = useRef<{ x: number; y: number } | null>(null);
  const timers = useRef<number[]>([]);

  const viewport = { width: window.innerWidth, height: window.innerHeight };
  const { side, gap } = useMemo(() => gridMetrics({ w: viewport.width, h: viewport.height }, grid.cols, grid.rows), [viewport.width, viewport.height, grid.cols, grid.rows]);
  const inner = popupSize(page, side, gap);
  const pad = 0.12 * side;
  const size = { width: inner.width + 2 * pad, height: inner.height + 2 * pad };
  const place = placePopup(anchor, size, viewport, SCREEN_PAD_PX);

  const requestClose = useCallback(() => {
    if (closingRef.current) return;
    closingRef.current = true;
    setClosing(true);
    timers.current.push(window.setTimeout(closePopup, CLOSE_ANIMATION_MS));
  }, []);

  useEffect(
    () => () => {
      timers.current.forEach((t) => window.clearTimeout(t));
    },
    [],
  );

  // Ten seconds without a touch closes the group; every touch starts the count again.
  useEffect(() => {
    let idle = window.setTimeout(requestClose, POPUP_IDLE_MS);
    const touch = () => {
      window.clearTimeout(idle);
      idle = window.setTimeout(requestClose, POPUP_IDLE_MS);
    };
    window.addEventListener("pointerdown", touch, true);
    return () => {
      window.clearTimeout(idle);
      window.removeEventListener("pointerdown", touch, true);
    };
  }, [requestClose]);

  // A press inside closes the group shortly after it succeeded, unless the page says `stayOpen`.
  const onPress = useCallback(
    (actionId: string) =>
      props.onPress(actionId).then(() => {
        if (page.stayOpen !== true) timers.current.push(window.setTimeout(requestClose, POPUP_CLOSE_DELAY_MS));
      }),
    [props, page.stayOpen, requestClose],
  );

  const origin = `${anchor.x + anchor.width / 2 - place.left}px ${anchor.y + anchor.height / 2 - place.top}px`;
  return (
    <div className={closing ? "popup-backdrop closing" : "popup-backdrop"} onPointerDown={requestClose}>
      <div
        className="tile panel popup-panel"
        style={{
          left: place.left,
          top: place.top,
          width: size.width,
          height: size.height,
          padding: pad,
          transformOrigin: origin,
          ["--side" as string]: `${side}px`,
        }}
        onPointerDown={(e) => {
          e.stopPropagation(); // a touch on the panel is not "outside"
          swipe.current = { x: e.clientX, y: e.clientY };
        }}
        onPointerMove={(e) => {
          const s = swipe.current;
          if (!s) return;
          const dy = e.clientY - s.y;
          if (dy >= POPUP_SWIPE_DOWN_PX && dy > Math.abs(e.clientX - s.x)) {
            swipe.current = null;
            requestClose();
          }
        }}
        onPointerUp={() => (swipe.current = null)}
        onPointerCancel={() => (swipe.current = null)}
      >
        <PopupGrid page={page} side={side} gap={gap} {...props} onPress={onPress} />
      </div>
    </div>
  );
}
