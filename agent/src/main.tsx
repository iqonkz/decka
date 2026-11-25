import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import App from "./App";
import HighlightApp from "./HighlightApp";
import IslandApp from "./IslandApp";
import "./styles.css";

// The island (voice and lyrics, T26.4) is a second window of the same bundle: `index.html?island=1`.
const params = new URLSearchParams(window.location.search);
const island = params.has("island");
// The highlight (T27.2) is a third one: `index.html?highlight=1`, a transparent click-through window over a monitor.
const highlight = params.has("highlight");

createRoot(document.getElementById("root")!).render(<StrictMode>{highlight ? <HighlightApp /> : island ? <IslandApp /> : <App />}</StrictMode>);