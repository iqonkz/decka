import { getLanguage, pickText, type CatalogItem } from "@decka/shared";
import { domainOf, formatChord } from "./addButton";

/** The file name of a program path: `C:\Apps\Spotify.exe` -> `Spotify.exe`. */
function exeName(program: string): string {
  const parts = program.split(/[\\/]/);
  return parts[parts.length - 1] ?? program;
}

/**
 * What an item does, in grey on the right of a library row: the keys of a shortcut, the name of the exe, the host of a link,
 * the size of a widget. Media, audio, OBS and system keys have none (the name says it); a hint equal to the name is dropped.
 */
export function hintOf(item: CatalogItem): string {
  const name = pickText(item.name, getLanguage());
  let hint = "";
  if (item.widget) hint = `${item.widget.w}×${item.widget.h}`;
  else if (item.action) {
    const a = item.action as { type: string; keys?: string[]; program?: string; url?: string };
    if (a.type === "hotkey" && a.keys && a.keys.length > 0) hint = formatChord(a.keys);
    else if (a.type === "launch" && a.program) hint = exeName(a.program);
    else if (a.type === "open_url" && a.url) hint = domainOf(a.url) ?? a.url;
  }
  return hint === name ? "" : hint;
}
