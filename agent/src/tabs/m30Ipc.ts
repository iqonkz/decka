// The calls of the Notes page and of the Home Assistant section (T30.1, T30.8). The caller is given in, so that the tests
// see what is sent. Nothing here swallows an error: the page shows it.
import type { HaStatus, NoteEntry } from "@decka/shared";
import type { call } from "../ipc";

type Call = typeof call;

export const loadNotes = (c: Call): Promise<NoteEntry[]> => c("notes_list");

/** Deletes the note on the PC; the list without it. Rejects (and the list stays) when the PC could not. */
export async function removeNote(c: Call, notes: NoteEntry[], id: string): Promise<NoteEntry[]> {
  await c("notes_delete", { id });
  return notes.filter((n) => n.id !== id);
}

/** Saves the address, the token (empty = keep the stored one) and the chosen devices; the state the PC has afterwards. */
export async function saveHa(c: Call, url: string, token: string, entities: HaStatus["entities"]): Promise<HaStatus> {
  await c("ha_set_config", { url: url.trim(), token: token.trim(), entities: entities.filter((e) => e.enabled).map((e) => e.id) });
  return c("ha_get");
}
