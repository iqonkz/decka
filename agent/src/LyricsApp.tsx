import { useEffect, useState } from "react";
import { listen } from "./ipc";

interface Lines {
  current: string | null;
  next: string | null;
}

/**
 * The root of the lyrics window (`index.html?lyrics`, DESIGN.md 4.17): two lines, the current and the next, on a dark pill.
 * The window is transparent and click-through; the PC sends `lyrics-lines` when the line changes.
 */
export default function LyricsApp() {
  const [lines, setLines] = useState<Lines>({ current: null, next: null });
  // Restarts the 200 ms entry animation on every change of the current line.
  const [tick, setTick] = useState(0);
  useEffect(() => {
    document.body.style.background = "transparent";
    document.documentElement.style.background = "transparent";
    let off: (() => void) | undefined;
    let live = true;
    void listen<Lines>("lyrics-lines", (event) => {
      setLines(event.payload);
      setTick((n) => n + 1);
    }).then((fn) => {
      if (live) off = fn;
      else fn();
    });
    return () => {
      live = false;
      off?.();
    };
  }, []);

  if (!lines.current && !lines.next) return null;
  return (
    <div className="lyrics-wrap" aria-hidden="true">
      <div className="lyrics-pill">
        <div key={tick} className="lyrics-current">
          {lines.current ?? " "}
        </div>
        {lines.next && <div className="lyrics-next">{lines.next}</div>}
      </div>
    </div>
  );
}
