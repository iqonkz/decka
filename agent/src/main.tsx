import { lazy, StrictMode, Suspense } from "react";
import { createRoot } from "react-dom/client";
import "./styles.css";

// The island (voice and lyrics, T26.4) is a second window of the same bundle: `index.html?island=1`.
const params = new URLSearchParams(window.location.search);
const island = params.has("island");
// The highlight (T27.2) is a third one: `index.html?highlight=1`, a transparent click-through window over a monitor.
const highlight = params.has("highlight");

// T31.5: each window loads only its own code.
const Window = lazy(() => (highlight ? import("./HighlightApp") : island ? import("./IslandApp") : import("./App")));

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <Suspense fallback={null}>
      <Window />
    </Suspense>
  </StrictMode>,
);
