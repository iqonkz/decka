/// <reference types="vite/client" />

interface ImportMetaEnv {
  readonly VITE_APP_VERSION: string;
  /** Optional `host:port` of the agent when the client is served by the Vite dev server. */
  readonly VITE_DEV_HOST?: string;
}
