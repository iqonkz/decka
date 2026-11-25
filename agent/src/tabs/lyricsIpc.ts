import type { LyricsPcOverlay } from "@decka/shared";
import { call } from "../ipc";

/** The Lyrics card as the PC stores it: the lookup switch and the mode of the lyrics window on the PC. */
export interface LyricsState {
  enabled: boolean;
  pcOverlay: LyricsPcOverlay;
}

/** The switch of the song lyrics (DATA_MODEL.md 14, `media.lyrics.enabled`, off by default) and `media.lyrics.pcOverlay`. */
export async function lyricsGet(): Promise<LyricsState> {
  return call("lyrics_get");
}

/** Switches the lookup on or off; off clears the lyrics on the phones at once. Returns what the PC stored. */
export async function lyricsSet(enabled: boolean): Promise<LyricsState> {
  return call("lyrics_set", { enabled });
}

/** Sets the mode of the lyrics window on the PC. Returns what the PC stored. */
export async function lyricsOverlaySet(mode: LyricsPcOverlay): Promise<LyricsState> {
  return call("lyrics_overlay_set", { mode });
}
