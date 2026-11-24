import { useCallback, useEffect, useState } from "react";
import { t, type ListRow, type ReminderRow } from "@decka/shared";
import { call } from "../ipc";
import Banner from "../ui/Banner";
import Button from "../ui/Button";
import ConfirmButton from "../ui/ConfirmButton";
import PhIcon from "../ui/PhIcon";
import { editItems, isLocalTime, nextHour, visibleReminders } from "./remindersModel";

type View = "reminders" | "lists";

/** A text that is changed in place: a click turns it into a field, Enter or leaving saves, Esc gives up. */
function InlineText({ value, label, done, onSave }: { value: string; label: string; done?: boolean; onSave: (text: string) => void }) {
  const [text, setText] = useState<string | null>(null);
  if (text === null) {
    return (
      <button type="button" className={done ? "link rem-edit rem-edit-done" : "link rem-edit"} aria-label={label} onClick={() => setText(value)}>
        {value}
      </button>
    );
  }
  const finish = (save: boolean) => {
    const next = text.trim();
    setText(null);
    if (save && next !== "" && next !== value) onSave(next);
  };
  return <input autoFocus className="grow" maxLength={200} aria-label={label} value={text} onChange={(e) => setText(e.target.value)} onBlur={() => finish(true)} onKeyDown={(e) => (e.key === "Enter" ? finish(true) : e.key === "Escape" ? finish(false) : undefined)} />;
}

function RemindersView({ onError }: { onError: (e: unknown) => void }) {
  const [rows, setRows] = useState<ReminderRow[]>([]);
  const [showDone, setShowDone] = useState(false);
  const [text, setText] = useState("");
  const [at, setAt] = useState(() => nextHour(new Date()));
  const load = useCallback(() => call("reminders_get").then(setRows, onError), [onError]);
  useEffect(() => {
    void load();
  }, [load]);
  const run = async (p: Promise<unknown>) => {
    try {
      await p;
    } catch (e) {
      onError(e);
    }
    await load();
  };
  const add = () => {
    if (text.trim() === "" || !isLocalTime(at)) return;
    void run(call("reminder_upsert", { text: text.trim(), at }).then(() => setText("")));
  };
  const shown = visibleReminders(rows, showDone);
  return (
    <section className="card">
      <label className="check">
        <input type="checkbox" checked={showDone} onChange={(e) => setShowDone(e.target.checked)} /> {t("rem.show_done")}
      </label>
      {shown.length === 0 && <p className="muted">{t("rem.empty")}</p>}
      {shown.map((r) => (
        <div className="row rem-line" key={r.id}>
          <input type="datetime-local" aria-label={t("rem.when")} value={r.at} onChange={(e) => isLocalTime(e.target.value) && void run(call("reminder_upsert", { id: r.id, text: r.text, at: e.target.value }))} />
          <InlineText value={r.text} label={t("rem.text")} done={r.done} onSave={(next) => void run(call("reminder_upsert", { id: r.id, text: next, at: r.at }))} />
          <span className="grow" />
          <Button className="btn-icon" aria-label={t("common.delete")} title={t("common.delete")} onClick={() => void run(call("reminder_delete", { id: r.id }))}>
            <PhIcon name="trash" size={16} />
          </Button>
        </div>
      ))}
      <div className="row rem-add">
        <input type="datetime-local" aria-label={t("rem.when")} value={at} onChange={(e) => setAt(e.target.value)} />
        <input className="grow" maxLength={200} placeholder={t("rem.add")} aria-label={t("rem.add")} value={text} onChange={(e) => setText(e.target.value)} onKeyDown={(e) => e.key === "Enter" && add()} />
        <Button variant="primary" disabled={text.trim() === "" || !isLocalTime(at)} onClick={add}>
          {t("rem.add_button")}
        </Button>
      </div>
    </section>
  );
}

function ListsView({ onError }: { onError: (e: unknown) => void }) {
  const [lists, setLists] = useState<ListRow[]>([]);
  const [current, setCurrent] = useState<string | null>(null);
  const [newItem, setNewItem] = useState("");
  const [newList, setNewList] = useState("");
  const load = useCallback(() => call("lists_get").then(setLists, onError), [onError]);
  useEffect(() => {
    void load();
  }, [load]);
  const run = async (p: Promise<unknown>) => {
    try {
      await p;
    } catch (e) {
      onError(e);
    }
    await load();
  };
  const list = lists.find((l) => l.id === current) ?? lists[0];
  const save = (l: ListRow, change: Parameters<typeof editItems>[1], name = l.name) => run(call("list_upsert", { id: l.id, name, items: editItems(l, change) }));
  const addList = async () => {
    const name = newList.trim();
    if (name === "") return;
    try {
      const made = await call("list_upsert", { name, items: [] });
      setCurrent(made.id);
      setNewList("");
    } catch (e) {
      onError(e);
    }
    await load();
  };
  return (
    <div className="rem-lists">
      <section className="card rem-names">
        {lists.length === 0 && <p className="muted">{t("rem.lists_empty")}</p>}
        {lists.map((l) => (
          <div className={l.id === list?.id ? "row rem-name on" : "row rem-name"} key={l.id}>
            <button type="button" className="link rem-edit grow" onClick={() => setCurrent(l.id)}>
              {l.name}
            </button>
            <ConfirmButton icon label={<PhIcon name="trash" size={16} />} title={t("common.delete")} question={t("rem.confirm_delete_list", { name: l.name })} onConfirm={() => void run(call("list_delete", { id: l.id }))} />
          </div>
        ))}
        <div className="row rem-add">
          <input className="grow" maxLength={200} placeholder={t("rem.add_list")} aria-label={t("rem.add_list")} value={newList} onChange={(e) => setNewList(e.target.value)} onKeyDown={(e) => e.key === "Enter" && void addList()} />
          <Button disabled={newList.trim() === ""} onClick={() => void addList()}>
            {t("rem.add_button")}
          </Button>
        </div>
      </section>
      <section className="card rem-items">
        {list && (
          <>
            <div className="row">
              <InlineText value={list.name} label={t("rem.list_name")} onSave={(name) => void save(list, {}, name)} />
            </div>
            {list.items.length === 0 && <p className="muted">{t("lists.empty")}</p>}
            {list.items.map((i) => (
              <div className="row rem-line" key={i.id}>
                <input type="checkbox" aria-label={i.text} checked={i.done} onChange={(e) => void save(list, { id: i.id, done: e.target.checked })} />
                <InlineText value={i.text} label={t("rem.text")} done={i.done} onSave={(text) => void save(list, { id: i.id, text })} />
                <span className="grow" />
                <Button className="btn-icon" aria-label={t("common.delete")} title={t("common.delete")} onClick={() => void save(list, { id: i.id, text: null })}>
                  <PhIcon name="trash" size={16} />
                </Button>
              </div>
            ))}
            <div className="row rem-add">
              <input className="grow" maxLength={200} placeholder={t("rem.add_item")} aria-label={t("rem.add_item")} value={newItem} onChange={(e) => setNewItem(e.target.value)} onKeyDown={(e) => e.key === "Enter" && newItem.trim() !== "" && void save(list, { text: newItem.trim() }).then(() => setNewItem(""))} />
              <Button disabled={newItem.trim() === ""} onClick={() => void save(list, { text: newItem.trim() }).then(() => setNewItem(""))}>
                {t("rem.add_button")}
              </Button>
            </div>
          </>
        )}
      </section>
    </div>
  );
}

/** `More -> Reminders` (DATA_MODEL.md 13.7): the reminders and the lists of the voice assistant, edited by hand. */
export default function RemindersTab() {
  const [view, setView] = useState<View>("reminders");
  const [error, setError] = useState<string | null>(null);
  const onError = useCallback((e: unknown) => setError(String(e)), []);
  return (
    <div className="stack">
      <h2>{t("more.reminders")}</h2>
      {error && <Banner kind="error">{error}</Banner>}
      <div className="seg" role="tablist">
        {(["reminders", "lists"] as const).map((v) => (
          <button key={v} type="button" role="tab" aria-selected={view === v} className={view === v ? "on" : ""} onClick={() => setView(v)}>
            {t(v === "reminders" ? "rem.tab_reminders" : "rem.tab_lists")}
          </button>
        ))}
      </div>
      {view === "reminders" ? <RemindersView onError={onError} /> : <ListsView onError={onError} />}
    </div>
  );
}