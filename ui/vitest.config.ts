import { defineConfig } from "vitest/config";

export default defineConfig({
  test: {
    // @material/material-color-utilities ships ESM with extensionless imports, which Node cannot resolve;
    // inlining makes Vite resolve them (the client bundle already does).
    server: { deps: { inline: ["@material/material-color-utilities"] } },
  },
});
