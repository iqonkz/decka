import { lazy, StrictMode, Suspense } from "react";
import { createRoot } from "react-dom/client";
import { removeM32Storage } from "./m32Cleanup";
import "./styles.css";

removeM32Storage(window.localStorage);

// The island (voice and lyrics, T26.4) is a second window of the same bundle: `index.html?island=1`.
const params = new URLSearchParams(window.location.search);
const island = params.has("island");
// The highlight (T27.2) is a third one: `index.html?highlight=1`, a transparent click-through window over a monitor.
const highlight = params.has("highlight");

// The mini panel of the tray (T32.3) is a fourth one: `index.html?tray=1`.
const tray = params.has("tray");

// T31.5: each window loads only its own code.
const Window = lazy(() => (highlight ? import("./HighlightApp") : island ? import("./IslandApp") : tray ? import("./TrayApp") : import("./App")));

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <Suspense fallback={null}>
      <Window />
    </Suspense>
  </StrictMode>,
);
