import { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";
import type { Page } from "@decka/shared";
import DeckBridge from "./DeckBridge";
import { navigableIndices } from "./popup";
import { dragOffset, SWIPE_START_PX, swipeDecision, targetPage, velocity } from "./swipe";

const SETTLE_MS = 220;
const SPEED_WINDOW_MS = 100;
const SETTLE_EASING = "cubic-bezier(0.2, 0.8, 0.2, 1)";

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

/** Runs `fn` when the browser is idle (a frame later at the latest); the fallback is a short timer. */
function whenIdle(fn: () => void): () => void {
  const w = window as Window & { requestIdleCallback?: (cb: () => void, o?: { timeout: number }) => number; cancelIdleCallback?: (id: number) => void };
  if (w.requestIdleCallback) {
    const id = w.requestIdleCallback(fn, { timeout: 200 });
    return () => w.cancelIdleCallback?.(id);
  }
  const id = window.setTimeout(fn, 50);
  return () => window.clearTimeout(id);
}

/**
 * The pages: the current one follows the finger, the neighbour slides in beside it, and the release settles in
 * 220 ms (DESIGN.md 3). Popup groups are not part of the swipe. A page that changes by itself (a button, a context
 * page) slides in from its side.
 *
 * The finger moves the layers straight in the DOM, at most once a frame (DATA_MODEL.md 10.2): React state changes only
 * when a gesture starts towards a neighbour and after the settling, so the pages are not rendered on every move.
 */
export default function PageStrip({ pages, pageIndex, grid, onCommit }: Props) {
  const root = useRef<HTMLDivElement>(null);
  const track = useRef<Track | null>(null);
  const layers = useRef(new Map<number, HTMLDivElement>());
  const shifts = useRef(new Map<number, number>()); // page index -> position relative to the current page (-1, 0, 1)
  const offset = useRef(0);
  const frame = useRef<number | null>(null);
  const [settling, setSettling] = useState(false);
  const settlingRef = useRef(false);
  const [lead, setLead] = useState<number | null>(null);
  const [slide, setSlide] = useState<{ dir: 1 | -1; key: number } | null>(null);
  // The page whose neighbours are mounted; it follows `pageIndex` one idle moment after the settling.
  const [ready, setReady] = useState(pageIndex);
  const previous = useRef(pageIndex);
  const ownCommit = useRef(false);
  const timer = useRef<number | null>(null);

  const nav = navigableIndices(pages);
  const count = nav.length;
  const position = Math.max(0, nav.indexOf(pageIndex));

  /** Writes the transform of every layer: the finger offset plus the place of the layer. */
  const paint = useCallback(() => {
    frame.current = null;
    const w = root.current?.clientWidth || window.innerWidth;
    layers.current.forEach((el, index) => {
      el.style.transform = `translate3d(${offset.current + (shifts.current.get(index) ?? 0) * w}px,0,0)`;
    });
  }, []);
  const schedule = useCallback(() => {
    if (frame.current === null) frame.current = requestAnimationFrame(paint);
  }, [paint]);
  const setTransition = (value: string) => layers.current.forEach((el) => (el.style.transition = value));

  // A change of page that did not come from a swipe here slides in from its side.
  useEffect(() => {
    if (ownCommit.current) ownCommit.current = false;
    else if (pageIndex !== previous.current) setSlide({ dir: pageIndex > previous.current ? 1 : -1, key: Date.now() });
    previous.current = pageIndex;
  }, [pageIndex]);

  // New neighbours are mounted in an idle moment after the settling, not in the frame where the animation ends.
  useEffect(() => {
    if (ready === pageIndex || settling) return;
    return whenIdle(() => setReady(pageIndex));
  }, [ready, pageIndex, settling]);

  // After every render the layers get their resting places (a freshly mounted layer has no transform yet).
  useLayoutEffect(() => {
    if (!settlingRef.current) paint();
  });

  useEffect(
    () => () => {
      if (timer.current !== null) window.clearTimeout(timer.current);
      if (frame.current !== null) cancelAnimationFrame(frame.current);
    },
    [],
  );

  const width = () => root.current?.clientWidth || window.innerWidth;

  function onDown(e: React.PointerEvent) {
    if (settlingRef.current) return;
    track.current = { x0: e.clientX, y0: e.clientY, horizontal: null, samples: [{ x: e.clientX, t: performance.now() }], dx: 0 };
  }

  function onMove(e: React.PointerEvent) {
    const t = track.current;
    if (!t || settlingRef.current) return;
    const dx = e.clientX - t.x0;
    const dy = e.clientY - t.y0;
    if (t.horizontal === null) {
      if (Math.abs(dx) >= SWIPE_START_PX && Math.abs(dx) > Math.abs(dy)) {
        t.horizontal = true;
        // The gesture is ours: keep the pointer and tell the layers they are about to move.
        try {
          (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
        } catch {
          /* a pointer that is already gone */
        }
        setTransition("none");
      } else if (Math.abs(dy) >= SWIPE_START_PX) t.horizontal = false; // a vertical gesture (a scrolling list) is not ours
    }
    if (!t.horizontal) return;
    t.dx = dx;
    t.samples.push({ x: e.clientX, t: performance.now() });
    if (t.samples.length > 8) t.samples.shift();
    offset.current = dragOffset(dx, position, count);
    schedule();
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
    settlingRef.current = true;
    setSettling(true);
    if (frame.current !== null) {
      cancelAnimationFrame(frame.current);
      frame.current = null;
    }
    setTransition(`transform ${SETTLE_MS}ms ${SETTLE_EASING}`);
    const moves = target !== pageIndex;
    offset.current = moves ? (targetPosition > position ? -width() : width()) : 0;
    paint();
    timer.current = window.setTimeout(() => {
      setTransition("none");
      settlingRef.current = false;
      offset.current = 0;
      if (moves) {
        ownCommit.current = true;
        onCommit(target);
      }
      setSettling(false);
      setLead(null);
    }, SETTLE_MS);
  }

  const current = pages[pageIndex] ?? pages[0];
  // Mounted: the current page and, once the neighbours are ready, the two beside it. While they are not ready, the page
  // that was current before the swipe stays mounted (when it is a neighbour), so that its DOM is kept.
  const wanted = new Map<number, number>();
  wanted.set(pageIndex, 0);
  const beforeIndex = nav[position - 1];
  const afterIndex = nav[position + 1];
  if (ready === pageIndex) {
    if (beforeIndex !== undefined) wanted.set(beforeIndex, -1);
    if (afterIndex !== undefined) wanted.set(afterIndex, 1);
  } else {
    if (ready === beforeIndex) wanted.set(ready, -1);
    if (ready === afterIndex) wanted.set(ready, 1);
  }
  shifts.current = wanted;
  const ordered = [...wanted.entries()].sort((a, b) => a[1] - b[1]);
  return (
    <div
      ref={root}
      className="pages"
      style={{ touchAction: "pan-y" }}
      onPointerDown={onDown}
      onPointerMove={onMove}
      onPointerUp={() => finish(true)}
      onPointerCancel={() => finish(false)}
    >
      {/* One keyed list: when the swipe commits, the neighbour layer becomes the current one with the same key, so React
          keeps its DOM instead of mounting the page again. */}
      {ordered.map(([index, shift]) => {
        const page = pages[index];
        if (!page) return null;
        const moving = settling || track.current?.horizontal === true || lead !== null;
        const isCurrent = index === pageIndex && page === current;
        const key = isCurrent && slide ? `${page.id}-${slide.key}` : page.id;
        const className = `page-layer${isCurrent && slide ? (slide.dir > 0 ? " from-right" : " from-left") : ""}`;
        return (
          <div
            key={key}
            ref={(el) => {
              if (el) layers.current.set(index, el);
              else layers.current.delete(index);
            }}
            className={className}
            style={{ willChange: moving ? "transform" : "auto", visibility: shift === 0 || lead === index ? "visible" : "hidden" }}
          >
            <DeckBridge page={page} grid={grid} />
          </div>
        );
      })}
    </div>
  );
}
