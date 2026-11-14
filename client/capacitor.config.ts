import type { CapacitorConfig } from "@capacitor/cli";

const config: CapacitorConfig = {
  appId: "app.decka.client",
  appName: "Decka",
  webDir: "dist",
  // "https" would make the WebView block ws:// as mixed content (ARCHITECTURE.md 6.5).
  server: { androidScheme: "http", cleartext: true },
};

export default config;
