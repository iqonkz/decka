import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { HighlightOverlay, type HighlightEvent } from "@decka/ui";
import { call } from "./ipc";

/**
 * The root of the highlight window (`index.html?highlight=1`, T27.2): a transparent, click-through window over a monitor. The agent
 * sends `highlight { rect, text, step?, total? }` (CSS pixels of this window) and `highlight_clear`; the page draws the frame and the
 * bubble. It decides nothing: when the highlight ends is the agent's business.
 */
export default function HighlightApp() {
  const [highlight, setHighlight] = useState<HighlightEvent | null>(null);
  const [accent, setAccent] = useState("#7c5cff");
  const [theme, setTheme] = useState<"light" | "dark">(window.matchMedia("(prefers-color-scheme: light)").matches ? "light" : "dark");

  useEffect(() => {
    document.body.style.background = "transparent";
    document.documentElement.style.background = "transparent";
    call("get_layout").then(
      (l) => setAccent(l.display.accent ?? "#7c5cff"),
      () => {},
    );
    const media = window.matchMedia("(prefers-color-scheme: light)");
    const onTheme = () => setTheme(media.matches ? "light" : "dark");
    media.addEventListener("change", onTheme);
    const offs: Array<() => void> = [];
    let live = true;
    void listen<HighlightEvent>("highlight", (e) => setHighlight(e.payload)).then((f) => (live ? offs.push(f) : f()));
    void listen("highlight_clear", () => setHighlight(null)).then((f) => (live ? offs.push(f) : f()));
    return () => {
      live = false;
      offs.forEach((f) => f());
      media.removeEventListener("change", onTheme);
    };
  }, []);

  return <HighlightOverlay highlight={highlight} theme={theme} accent={accent} viewport={{ width: window.innerWidth, height: window.innerHeight }} />;
}
