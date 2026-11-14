import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import pkg from "./package.json";

export default defineConfig({
  plugins: [react()],
  base: "./",
  define: { "import.meta.env.VITE_APP_VERSION": JSON.stringify(pkg.version) },
  server: { port: 5173, strictPort: true, host: true },
  build: { target: "chrome90", outDir: "dist", emptyOutDir: true },
});
