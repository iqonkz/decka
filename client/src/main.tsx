import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import App from "./App";
import { init } from "./app/controller";
import { keepAwake } from "./platform/native";
import "./styles.css";

init();
keepAwake();

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
