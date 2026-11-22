import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import App from "./App";
import PillApp from "./PillApp";
import "./styles.css";

// The voice pill is a second window of the same bundle: `index.html?pill`.
const pill = new URLSearchParams(window.location.search).has("pill");

createRoot(document.getElementById("root")!).render(
  <StrictMode>{pill ? <PillApp /> : <App />}</StrictMode>,
);
