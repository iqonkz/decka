import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import App from "./App";
import { init } from "./app/controller";
import "./styles.css";

init();

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
