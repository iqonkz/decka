import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import App from "./App";
import IslandApp from "./IslandApp";
import "./styles.css";

// The island (voice and lyrics, T26.4) is a second window of the same bundle: `index.html?island=1`.
const island = new URLSearchParams(window.location.search).has("island");

createRoot(document.getElementById("root")!).render(<StrictMode>{island ? <IslandApp /> : <App />}</StrictMode>);