import { invoke } from "@tauri-apps/api/core";
import type { ConfigFile } from "@decka/shared";

/**
 * IPC helpers for M24 lyrics toggle.
 * TODO(T24.2): move `lyrics_get` / `lyrics_set` into `@decka/shared` `IpcCommands` once Rust backend adds them.
 */
export async function lyricsGet(fallbackConfig?: ConfigFile | null): Promise<boolean> {
  const fromConfig = Boolean(
    (fallbackConfig as { media?: { lyrics?: { enabled?: boolean } } } | null | undefined)?.media?.lyrics?.enabled,
  );
  try {
    const res = await invoke<boolean | { enabled?: boolean } | null>("lyrics_get");
    if (typeof res === "boolean") return res;
    if (res && typeof res === "object" && typeof res.enabled === "boolean") return res.enabled;
    return fromConfig;
  } catch {
    return fromConfig;
  }
}

/**
 * Persists `media.lyrics.enabled` via `lyrics_set` IPC when available.
 * Returns true if stored or gracefully falls back when the command is not yet registered in Rust.
 * TODO(T24.2): remove fallback once Rust registers `lyrics_set`.
 */
export async function lyricsSet(enabled: boolean): Promise<boolean> {
  try {
    await invoke<unknown>("lyrics_set", { enabled });
    return true;
  } catch (err) {
    const msg = String(err ?? "");
    if (/unknown command|not found|did not find/i.test(msg)) {
      return true;
    }
    throw err;
  }
}
