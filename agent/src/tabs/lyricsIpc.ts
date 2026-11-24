import { call } from "../ipc";

/** The switch of the song lyrics (DATA_MODEL.md 14, `media.lyrics.enabled`, off by default) through the typed IPC of the PC. */
export async function lyricsGet(): Promise<boolean> {
  return (await call("lyrics_get")).enabled;
}

/** Switches the lookup on or off; off clears the lyrics on the phones at once. Returns what the PC stored. */
export async function lyricsSet(enabled: boolean): Promise<boolean> {
  return (await call("lyrics_set", { enabled })).enabled;
}
