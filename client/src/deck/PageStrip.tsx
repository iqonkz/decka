import { useEffect, useRef, useState } from "react";
import type { Page } from "@decka/shared";
import DeckBridge from "./DeckBridge";
import { navigableIndices } from "./popup";
import { dragOffset, SWIPE_START_PX, swipeDecision, targetPage, velocity } from "./swipe";

const SETTLE_MS = 220;
const SPEED_WINDOW_MS = 100;

interface Track {
  x0: number;
  y0: number;
  horizontal: boolean | null;
  samples: { x: number; t: number }[];
  dx: number;
}

interface Props {
  pages: Page[];
  /** Index of the current page in `pages`. */
  pageIndex: number;
  grid: { cols: number; rows: number };
  /** The user finished a swipe on this page; gets the index in `pages`. */
  onCommit: (index: number) => void;
}

/**
 * The pages: the current one follows the finger, the neighbour slides in beside it, and the release settles in
 * 220 ms (DESIGN.md 3). Popup groups are not part of the swipe. A page that changes by itself (a button, a context
 * page) slides in from its side.
 */
export default function PageStrip({ pages, pageIndex, grid, onCommit }: Props) {
  const root = useRef<HTMLDivElement>(null);
  const track = useRef<Track | null>(null);
  const [offset, setOffset] = useState(0);
  const [settling, setSettling] = useState(false);
  const [lead, setLead] = useState<number | null>(null);
  const [slide, setSlide] = useState<{ dir: 1 | -1; key: number } | null>(null);
  const previous = useRef(pageIndex);
  const ownCommit = useRef(false);
  const timer = useRef<number | null>(null);

  const nav = navigableIndices(pages);
  const count = nav.length;
  const position = Math.max(0, nav.indexOf(pageIndex));

  // A change of page that did not come from a swipe here slides in from its side.
  useEffect(() => {
    if (ownCommit.current) ownCommit.current = false;
    else if (pageIndex !== previous.current) setSlide({ dir: pageIndex > previous.current ? 1 : -1, key: Date.now() });
    previous.current = pageIndex;
  }, [pageIndex]);

  useEffect(
    () => () => {
      if (timer.current !== null) window.clearTimeout(timer.current);
    },
    [],
  );

  const width = () => root.current?.clientWidth || window.innerWidth;

  function onDown(e: React.PointerEvent) {
    if (settling) return;
    track.current = { x0: e.clientX, y0: e.clientY, horizontal: null, samples: [{ x: e.clientX, t: performance.now() }], dx: 0 };
  }

  function onMove(e: React.PointerEvent) {
    const t = track.current;
    if (!t || settling) return;
    const dx = e.clientX - t.x0;
    const dy = e.clientY - t.y0;
    if (t.horizontal === null) {
      if (Math.abs(dx) >= SWIPE_START_PX && Math.abs(dx) > Math.abs(dy)) t.horizontal = true;
      else if (Math.abs(dy) >= SWIPE_START_PX) t.horizontal = false; // a vertical gesture (a scrolling list) is not ours
    }
    if (!t.horizontal) return;
    t.dx = dx;
    t.samples.push({ x: e.clientX, t: performance.now() });
    if (t.samples.length > 8) t.samples.shift();
    setOffset(dragOffset(dx, position, count));
    const neighbour = nav[dx < 0 ? position + 1 : position - 1];
    setLead(neighbour ?? null);
  }

  function finish(commit: boolean) {
    const t = track.current;
    track.current = null;
    if (!t || !t.horizontal) return;
    const now = performance.now();
    const recent = t.samples.filter((s) => now - s.t <= SPEED_WINDOW_MS);
    const first = recent[0] ?? t.samples[0];
    const last = t.samples[t.samples.length - 1];
    const v = first && last ? velocity(first.x, first.t, last.x, last.t) : 0;
    const decision = commit ? swipeDecision(t.dx, v, width()) : "stay";
    const targetPosition = targetPage(position, decision, count);
    const target = nav[targetPosition] ?? pageIndex;
    setSettling(true);
    if (target !== pageIndex) {
      setOffset(targetPosition > position ? -width() : width());
      timer.current = window.setTimeout(() => {
        ownCommit.current = true;
        onCommit(target);
        setSettling(false);
        setOffset(0);
        setLead(null);
      }, SETTLE_MS);
    } else {
      setOffset(0);
      timer.current = window.setTimeout(() => {
        setSettling(false);
        setLead(null);
      }, SETTLE_MS);
    }
  }

  const transition = settling ? `transform ${SETTLE_MS}ms cubic-bezier(0.2, 0.8, 0.2, 1)` : "none";
  const current = pages[pageIndex] ?? pages[0];
  const neighbour = lead !== null ? pages[lead] : undefined;
  return (
    <div ref={root} className="pages" onPointerDown={onDown} onPointerMove={onMove} onPointerUp={() => finish(true)} onPointerCancel={() => finish(false)}>
      {current && (
        <div
          key={slide ? `${current.id}-${slide.key}` : current.id}
          className={`page-layer${slide ? (slide.dir > 0 ? " from-right" : " from-left") : ""}`}
          style={{ transform: `translateX(${offset}px)`, transition }}
        >
          <DeckBridge page={current} grid={grid} />
        </div>
      )}
      {neighbour && lead !== null && (
        <div className="page-layer" style={{ transform: `translateX(${offset + (lead > pageIndex ? width() : -width())}px)`, transition }}>
          <DeckBridge page={neighbour} grid={grid} />
        </div>
      )}
    </div>
  );
}
