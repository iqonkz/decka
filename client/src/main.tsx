import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { resolveLanguage, setLanguage } from "@decka/shared";
import App from "./App";
import { init } from "./app/controller";
import { keepAwake } from "./platform/native";
import "./styles.css";

// Before the first render the language is the system one; the layout's `display.language` replaces it later (App).
setLanguage(resolveLanguage("auto", navigator.language));
init();
keepAwake();

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
