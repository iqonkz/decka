import { useState } from "react";
import { t, type ListItem } from "@decka/shared";
import { useDeckEnv } from "../deckEnv";
import Icon from "../Icon";
import { activeGuesses, listRows, NO_GUESS, tapped } from "./listLogic";

/**
 * One list with tick marks (DATA_MODEL.md 13.7): a ring with a tick and the text; done items are struck through, half
 * transparent and last. A tap sends `list_toggle` and shows the new mark at once; the next patch of the `lists` section
 * replaces the guess (so a refused toggle rolls back by itself).
 */
export default function ListWidget({ item }: { item: ListItem }) {
  const { lists, onToggleListItem, online } = useDeckEnv();
  const [guess, setGuess] = useState(NO_GUESS);
  const all = lists?.lists ?? [];
  // A widget without a list shows the first one; a deleted list is reported.
  const list = item.listId === undefined ? all[0] : all.find((l) => l.id === item.listId);
  if (list === undefined) {
    return (
      <div className="rem">
        <div className="rem-title">{t("lists.title")}</div>
        <div className="rem-none">{item.listId === undefined ? t("lists.empty") : t("lists.not_found")}</div>
      </div>
    );
  }
  const rows = listRows(list.items, activeGuesses(guess, lists));
  const toggle = (itemId: string, done: boolean) => {
    if (!online || !onToggleListItem) return;
    setGuess(tapped(guess, lists, itemId, done));
    onToggleListItem(list.id, itemId);
  };
  return (
    <div className="rem">
      <div className="rem-title">{list.name}</div>
      {rows.length === 0 ? (
        <div className="rem-none">{t("lists.empty")}</div>
      ) : (
        rows.slice(0, Math.max(1, item.h * 2 - 1)).map((r) => (
          <div className={r.done ? "rem-row list-done" : "rem-row"} key={r.id} onClick={() => toggle(r.id, r.done)}>
            <span className="list-mark">{r.done ? <Icon icon="lucide:check" size={14} /> : null}</span>
            <span className="rem-text">{r.text}</span>
          </div>
        ))
      )}
    </div>
  );
}