import { useCallback, useEffect, useState } from "react";
import { t, type ListRow, type ReminderRepeat, type ReminderRow } from "@decka/shared";
import { formatRepeatLabel } from "@decka/ui";
import { call } from "../ipc";
import Banner from "../ui/Banner";
import Button from "../ui/Button";
import Checkbox from "../ui/Checkbox";
import ConfirmButton from "../ui/ConfirmButton";
import PhIcon from "../ui/PhIcon";
import { editItems, isLocalTime, nextHour, visibleReminders } from "./remindersModel";
import { Group, PageHeader, Row, SettingsEmpty } from "../ui/SettingsGroup";
import Switch from "../ui/Switch";
import Segmented from "../ui/Segmented";
import TimePickerPopover, { formatReminderLabel, isOverdue } from "../ui/TimePickerPopover";

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


function ReminderRowItem({
  reminder,
  onSave,
  onDelete,
}: {
  reminder: ReminderRow;
  onSave: (patch: { text?: string; at?: string; repeat?: ReminderRepeat | null }) => void;
  onDelete: () => void;
}) {
  const [showPicker, setShowPicker] = useState(false);
  const overdue = !reminder.done && isOverdue(reminder.at);
  const repeatLabel = formatRepeatLabel(reminder.repeat, reminder.at);

  return (
    <Row
      key={reminder.id}
      title={
        <div className="rem-row-info">
          <InlineText
            value={reminder.text}
            label={t("rem.text")}
            done={reminder.done}
            onSave={(next) => onSave({ text: next })}
          />
          <div className="rem-row-meta">
            <div className="rem-pill-wrap">
              <button
                type="button"
                className={`rem-time-pill ${overdue ? "overdue" : ""}`}
                onClick={() => setShowPicker(!showPicker)}
              >
                <PhIcon name="clock" size={13} />
                <span>{formatReminderLabel(reminder.at)}</span>
              </button>
              {showPicker && (
                <TimePickerPopover
                  value={reminder.at}
                  onChange={(nextAt) => onSave({ at: nextAt })}
                  repeat={reminder.repeat}
                  onRepeatChange={(nextRep) => onSave({ repeat: nextRep ?? null })}
                  onClose={() => setShowPicker(false)}
                />
              )}
            </div>
            {repeatLabel && (
              <span className="rem-repeat-badge">
                <PhIcon name="repeat" size={12} />
                <span>{repeatLabel}</span>
              </span>
            )}
          </div>
        </div>
      }
    >
      <Button
        className="btn-icon"
        aria-label={t("common.delete")}
        title={t("common.delete")}
        onClick={onDelete}
      >
        <PhIcon name="trash" size={16} />
      </Button>
    </Row>
  );
}

function ReminderAddRow({ onAdd }: { onAdd: (text: string, at: string, repeat?: ReminderRepeat) => void }) {
  const [text, setText] = useState("");
  const [at, setAt] = useState(() => nextHour(new Date()));
  const [repeat, setRepeat] = useState<ReminderRepeat | undefined>(undefined);
  const [showPicker, setShowPicker] = useState(false);
  const repeatLabel = formatRepeatLabel(repeat, at);

  const add = () => {
    if (text.trim() === "" || !isLocalTime(at)) return;
    onAdd(text.trim(), at, repeat);
    setText("");
    setAt(nextHour(new Date()));
    setRepeat(undefined);
  };

  return (
    <div className="rem-add-row" style={{ padding: "14px 0" }}>
      <input
        className="rem-add-text grow"
        maxLength={200}
        placeholder={t("rem.add")}
        aria-label={t("rem.add")}
        value={text}
        onChange={(e) => setText(e.target.value)}
        onKeyDown={(e) => e.key === "Enter" && add()}
      />
      <div className="rem-pill-wrap">
        <button
          type="button"
          className="rem-time-pill"
          onClick={() => setShowPicker(!showPicker)}
        >
          <PhIcon name="clock" size={13} />
          <span>{formatReminderLabel(at)}</span>
          {repeatLabel && (
            <>
              <PhIcon name="repeat" size={12} />
              <span>{repeatLabel}</span>
            </>
          )}
        </button>
        {showPicker && (
          <TimePickerPopover
            value={at}
            onChange={(nextAt) => setAt(nextAt)}
            repeat={repeat}
            onRepeatChange={setRepeat}
            onClose={() => setShowPicker(false)}
          />
        )}
      </div>
      <Button variant="primary" disabled={text.trim() === "" || !isLocalTime(at)} onClick={add}>
        {t("rem.add_button")}
      </Button>
    </div>
  );
}

function RemindersView({ onError }: { onError: (e: unknown) => void }) {
  const [rows, setRows] = useState<ReminderRow[]>([]);
  const [showDone, setShowDone] = useState(false);
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
  const shown = visibleReminders(rows, showDone) as ReminderRow[];
  return (
    <div id="reminders-list" style={{ marginTop: 16 }}>
      <Group>
        <Row title={t("rem.show_done")}>
          <Switch checked={showDone} onChange={setShowDone} />
        </Row>
        {shown.length === 0 ? (
          <div style={{ padding: "14px 0" }}>
            <SettingsEmpty>{t("rem.empty")}</SettingsEmpty>
          </div>
        ) : (
          shown.map((r) => (
            <ReminderRowItem
              key={r.id}
              reminder={r}
              onSave={(patch) => {
                // Without repeat the series stays, "once" ends it (the PC reads it so).
                const repeat = patch.repeat === null ? "once" : (patch.repeat ?? r.repeat);
                void run(
                  call("reminder_upsert", {
                    id: r.id,
                    text: patch.text ?? r.text,
                    at: patch.at ?? r.at,
                    ...(repeat ? { repeat } : {}),
                  }),
                );
              }}
              onDelete={() => void run(call("reminder_delete", { id: r.id }))}
            />
          ))
        )}
        <ReminderAddRow
          onAdd={(addText, addAt, addRepeat) =>
            void run(call("reminder_upsert", { text: addText, at: addAt, ...(addRepeat ? { repeat: addRepeat } : {}) }))
          }
        />
      </Group>
    </div>
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
    <div id="reminders-items" className="rem-lists" style={{ marginTop: 16 }}>
      <Group className="rem-names">
        {lists.length === 0 && <p className="muted" style={{ padding: "14px 0" }}>{t("rem.lists_empty")}</p>}
        {lists.map((l) => (
          <Row
            key={l.id}
            title={
              <button type="button" className={l.id === list?.id ? "link rem-edit grow on" : "link rem-edit grow"} onClick={() => setCurrent(l.id)}>
                {l.name}
              </button>
            }
          >
            <ConfirmButton icon label={<PhIcon name="trash" size={16} />} title={t("common.delete")} question={t("rem.confirm_delete_list", { name: l.name })} onConfirm={() => void run(call("list_delete", { id: l.id }))} />
          </Row>
        ))}
        <div className="row rem-add" style={{ padding: "14px 0" }}>
          <input className="grow" maxLength={200} placeholder={t("rem.add_list")} aria-label={t("rem.add_list")} value={newList} onChange={(e) => setNewList(e.target.value)} onKeyDown={(e) => e.key === "Enter" && void addList()} />
          <Button disabled={newList.trim() === ""} onClick={() => void addList()}>
            {t("rem.add_button")}
          </Button>
        </div>
      </Group>

      <Group className="rem-items">
        {list ? (
          <>
            <div className="row" style={{ padding: "14px 0" }}>
              <InlineText value={list.name} label={t("rem.list_name")} onSave={(name) => void save(list, {}, name)} />
            </div>
            {list.items.length === 0 && <p className="muted" style={{ padding: "8px 0" }}>{t("lists.empty")}</p>}
            {list.items.map((i) => (
              <Row
                key={i.id}
                title={
                  <div className="row" style={{ flexWrap: "wrap" }}>
                    <Checkbox aria-label={i.text} checked={i.done} onChange={(done) => void save(list, { id: i.id, done })} />
                    <InlineText value={i.text} label={t("rem.text")} done={i.done} onSave={(text) => void save(list, { id: i.id, text })} />
                  </div>
                }
              >
                <Button className="btn-icon" aria-label={t("common.delete")} title={t("common.delete")} onClick={() => void save(list, { id: i.id, text: null })}>
                  <PhIcon name="trash" size={16} />
                </Button>
              </Row>
            ))}
            <div className="row rem-add" style={{ padding: "14px 0" }}>
              <input className="grow" maxLength={200} placeholder={t("rem.add_item")} aria-label={t("rem.add_item")} value={newItem} onChange={(e) => setNewItem(e.target.value)} onKeyDown={(e) => e.key === "Enter" && newItem.trim() !== "" && void save(list, { text: newItem.trim() }).then(() => setNewItem(""))} />
              <Button disabled={newItem.trim() === ""} onClick={() => void save(list, { text: newItem.trim() }).then(() => setNewItem(""))}>
                {t("rem.add_button")}
              </Button>
            </div>
          </>
        ) : (
          <div style={{ padding: "14px 0" }}>
            <SettingsEmpty>{t("rem.lists_empty")}</SettingsEmpty>
          </div>
        )}
      </Group>
    </div>
  );
}

/** `More -> Reminders` (DATA_MODEL.md 13.7): the reminders and the lists of the voice assistant, edited by hand. */
export default function RemindersTab() {
  const [view, setView] = useState<View>("reminders");
  const [error, setError] = useState<string | null>(null);
  const onError = useCallback((e: unknown) => setError(String(e)), []);
  return (
    <div className="page-container">
      <PageHeader icon="bell" title={t("more.reminders")} />
      {error && <Banner kind="error">{error}</Banner>}
      <Segmented
        options={[
          { value: "reminders", label: t("rem.tab_reminders") },
          { value: "lists", label: t("rem.tab_lists") },
        ]}
        value={view}
        onChange={setView}
      />
      {view === "reminders" ? <RemindersView onError={onError} /> : <ListsView onError={onError} />}
    </div>
  );
}
