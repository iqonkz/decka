import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import App from "./App";
import LyricsApp from "./LyricsApp";
import PillApp from "./PillApp";
import "./styles.css";

// The voice pill is a second window of the same bundle: `index.html?pill`.
const params = new URLSearchParams(window.location.search);
const pill = params.has("pill");
// The lyrics window (0.30) is another one: `index.html?lyrics`.
const lyrics = params.has("lyrics");

createRoot(document.getElementById("root")!).render(
  <StrictMode>{lyrics ? <LyricsApp /> : pill ? <PillApp /> : <App />}</StrictMode>,
);
