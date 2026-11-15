import { defineConfig } from "vitest/config";

export default defineConfig({
  test: {
    // @decka/ui pulls in @material/material-color-utilities, whose ESM has extensionless imports that Node cannot
    // resolve; inlining lets Vite resolve them (the production bundle does the same).
    server: { deps: { inline: ["@material/material-color-utilities"] } },
  },
});
