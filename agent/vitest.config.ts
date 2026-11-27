import { defineConfig, mergeConfig } from "vitest/config";
import vite from "./vite.config";

export default mergeConfig(
  vite,
  defineConfig({
    test: {
      // As in ui/: @material/material-color-utilities ships ESM with extensionless imports, which Node cannot resolve.
      server: { deps: { inline: ["@material/material-color-utilities"] } },
    },
  }),
);
