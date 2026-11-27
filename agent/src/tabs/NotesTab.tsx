import { useEffect, useMemo, useState } from "react";
import { t, type NoteEntry } from "@decka/shared";
import { call } from "../ipc";
import Banner from "../ui/Banner";
import Button from "../ui/Button";
import PhIcon from "../ui/PhIcon";
import { Group, PageHeader, Row } from "../ui/SettingsGroup";
import Switch from "../ui/Switch";
import { loadNotes, removeNote } from "./m30Ipc";

export type { NoteEntry };

export interface NotesTabProps {
  initialNotes?: NoteEntry[];
}

function renderMarkdownBlocks(md: string) {
  const lines = md.split(/\r?\n/);
  return lines.map((line, i) => {
    if (line.startsWith("### ")) return <h4 key={i} style={{ margin: "10px 0 4px" }}>{line.slice(4)}</h4>;
    if (line.startsWith("## ")) return <h3 key={i} style={{ margin: "12px 0 6px" }}>{line.slice(3)}</h3>;
    if (line.startsWith("# ")) return <h2 key={i} style={{ margin: "0 0 8px", fontSize: 18 }}>{line.slice(2)}</h2>;
    if (line.startsWith("- ")) return <div key={i} style={{ paddingInlineStart: 12, margin: "2px 0" }}>• {line.slice(2)}</div>;
    if (line.trim() === "") return <div key={i} style={{ height: 6 }} />;
    return <p key={i} style={{ margin: "4px 0", lineHeight: 1.5 }}>{line}</p>;
  });
}

/** T30.1b: Sidebar page for local Markdown meeting notes. */
export default function NotesTab({ initialNotes }: NotesTabProps = {}) {
  const [notes, setNotes] = useState<NoteEntry[]>(initialNotes ?? []);
  const [selectedId, setSelectedId] = useState<string | null>(initialNotes?.[0]?.id ?? null);
  const [query, setQuery] = useState("");
  const [confirmDeleteId, setConfirmDeleteId] = useState<string | null>(null);
  const [keepAudio, setKeepAudio] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (initialNotes) return;
    let active = true;
    const failed = (e: unknown) => active && setError(String(e));
    const load = () => void loadNotes(call).then((list) => active && setNotes(list), failed);
    load();
    call("notes_keep_audio_get").then((on) => active && setKeepAudio(on), failed);
    // A note that was written while the window was away appears when it is looked at again.
    window.addEventListener("focus", load);
    return () => {
      active = false;
      window.removeEventListener("focus", load);
    };
  }, [initialNotes]);

  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase();
    if (!q) return notes;
    return notes.filter((n) => n.title.toLowerCase().includes(q) || n.markdown.toLowerCase().includes(q));
  }, [notes, query]);

  const selected = useMemo(
    () => filtered.find((n) => n.id === selectedId) ?? filtered[0] ?? null,
    [filtered, selectedId],
  );

  function handleDelete(id: string) {
    setError(null);
    setConfirmDeleteId(null);
    removeNote(call, notes, id).then(
      (left) => {
        setNotes(left);
        if (selectedId === id) setSelectedId(null);
      },
      (e) => setError(String(e)),
    );
  }

  function changeKeep(next: boolean) {
    setError(null);
    setKeepAudio(next);
    call("notes_keep_audio_set", { keep: next }).then(setKeepAudio, (e) => {
      setKeepAudio(!next);
      setError(String(e));
    });
  }

  return (
    <div className="page-container">
      <PageHeader icon="notepad" title={t("more.notes")} subtitle={t("m30.notes_subtitle")} />
      {error && <Banner kind="error">{error}</Banner>}

      <Group>
        <Row id="section-notes-keep" title={t("notes.keep_audio")} description={t("notes.keep_audio_hint")}>
          <Switch checked={keepAudio} onChange={changeKeep} />
        </Row>
      </Group>

      <div style={{ display: "flex", gap: 12, alignItems: "center", margin: "16px 0", flexWrap: "wrap" }}>
        <div className="integrations-search" style={{ flex: 1, marginBottom: 0 }}>
          <PhIcon name="magnifying-glass" size={16} />
          <input
            type="search"
            className="integrations-search-input"
            aria-label={t("m30.notes_search")}
            placeholder={t("m30.notes_search")}
            value={query}
            onChange={(e) => setQuery(e.target.value)}
          />
        </div>
        <Button onClick={() => void call("notes_open_folder").catch((e) => setError(String(e)))}>
          <PhIcon name="folder-open" size={15} />
          {t("m30.notes_open_folder")}
        </Button>
      </div>

      {filtered.length === 0 ? (
        <Group>
          <div style={{ padding: "20px 16px", fontSize: "var(--font-sm)", color: "var(--muted)" }}>
            {t("m30.notes_empty")}
          </div>
        </Group>
      ) : (
        <div style={{ display: "grid", gridTemplateColumns: "260px 1fr", gap: 16, alignItems: "start" }}>
          <Group>
            {filtered.map((n, idx) => {
              const active = selected?.id === n.id;
              return (
                <button
                  key={n.id}
                  type="button"
                  onClick={() => {
                    setSelectedId(n.id);
                    setConfirmDeleteId(null);
                  }}
                  style={{
                    width: "100%",
                    textAlign: "start",
                    padding: "10px 14px",
                    border: 0,
                    borderTop: idx > 0 ? "1px solid var(--border)" : undefined,
                    background: active ? "color-mix(in srgb, var(--accent) 12%, transparent)" : "transparent",
                    color: "var(--text)",
                    cursor: "pointer",
                    display: "flex",
                    flexDirection: "column",
                    gap: 3,
                  }}
                >
                  <strong style={{ fontSize: 13 }}>{n.title}</strong>
                  <span style={{ fontSize: 11, color: "var(--muted)" }}>{n.date}</span>
                </button>
              );
            })}
          </Group>

          {selected ? (
            <Group>
              <div style={{ padding: "14px 16px" }}>
                <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 12, marginBottom: 12, borderBottom: "1px solid var(--border)", paddingBottom: 10 }}>
                  <div>
                    <div style={{ fontWeight: 600, fontSize: 15 }}>{selected.title}</div>
                    <div style={{ fontSize: 12, color: "var(--muted)" }}>{selected.date}</div>
                  </div>
                  {confirmDeleteId === selected.id ? (
                    <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
                      <span style={{ fontSize: 12, color: "var(--danger)" }}>
                        {t("m30.notes_delete_confirm", { title: selected.title })}
                      </span>
                      <Button variant="danger" onClick={() => handleDelete(selected.id)}>
                        {t("m30.notes_delete")}
                      </Button>
                      <Button onClick={() => setConfirmDeleteId(null)}>{t("common.cancel")}</Button>
                    </div>
                  ) : (
                    <Button variant="danger" onClick={() => setConfirmDeleteId(selected.id)}>
                      {t("m30.notes_delete")}
                    </Button>
                  )}
                </div>
                <div className="notes-markdown" style={{ fontSize: 13 }}>
                  {renderMarkdownBlocks(selected.markdown)}
                </div>
              </div>
            </Group>
          ) : (
            <Group>
              <div style={{ padding: "20px 16px", fontSize: "var(--font-sm)", color: "var(--muted)" }}>
                {t("m30.notes_select")}
              </div>
            </Group>
          )}
        </div>
      )}
    </div>
  );
}
