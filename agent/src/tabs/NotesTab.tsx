import { Fragment, useEffect, useMemo, useState, type ReactNode } from "react";
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

// The heading of the full text in a note file: the words of `notes::text::words` in the 11 languages, and a hand-written one.
const TRANSCRIPT_HEADING =
  /^#{1,3}\s+(полный\s+текст|full\s+text|texto\s+completo|volltext|texte\s+intégral|全文|पूरा\s+पाठ|النص\s+الكامل|толық\s+мәтін|расшифровка|транскрипт|transcript)\s*:?\s*$/i;
// The line under the title that the agent writes: `2026-11-27 14:05 · 12 мин`.
const WHEN_LINE = /^\d{4}-\d{2}-\d{2} \d{2}:\d{2} · (\d+) \S+$/;
const DURATION_LINE = /^(?:[-*]\s*)?(?:длительность|продолжительность|duration)\s*:\s*(?:(\d+)\s*(?:мин(?:ут[ыа]?)?|min(?:utes?)?)|(\d{1,2}):(\d{2}))/i;

export function extractNoteDurationMin(markdown: string): number | null {
  for (const raw of markdown.split(/\r?\n/)) {
    const line = raw.trim();
    if (TRANSCRIPT_HEADING.test(line)) break;
    const m = line.match(WHEN_LINE) ?? line.match(DURATION_LINE);
    if (!m) continue;
    const minutes = m[1] ?? m[2];
    if (minutes) return Math.max(1, Number(minutes));
  }
  return null;
}

/**
 * Everything before the heading of the full text is the summary (with the decisions, tasks and questions); the full text is
 * folded. The line with the date and the length is left out: the header of the note shows both.
 */
export function splitNoteSections(markdown: string): { summaryMd: string; transcriptMd: string } {
  const lines = markdown.split(/\r?\n/);
  const found = lines.findIndex((line) => TRANSCRIPT_HEADING.test(line.trim()));
  const cut = found < 0 ? lines.length : found;
  const head = lines.slice(0, cut).filter((line) => !WHEN_LINE.test(line.trim()) && !DURATION_LINE.test(line.trim()));
  return {
    summaryMd: head.join("\n").trim(),
    transcriptMd: lines.slice(cut).join("\n").trim(),
  };
}

function renderMarkdownBlocks(md: string): ReactNode {
  const lines = md.split(/\r?\n/);
  const blocks: ReactNode[] = [];
  let listItems: string[] = [];

  const flushList = (key: string) => {
    if (listItems.length === 0) return;
    blocks.push(
      <ul key={key}>
        {listItems.map((item, j) => (
          <li key={j}>{item}</li>
        ))}
      </ul>,
    );
    listItems = [];
  };

  lines.forEach((line, i) => {
    if (line.startsWith("- ") || line.startsWith("* ")) {
      listItems.push(line.slice(2));
      return;
    }
    flushList(`ul-${i}`);
    if (line.startsWith("### ")) blocks.push(<h4 key={i}>{line.slice(4)}</h4>);
    else if (line.startsWith("## ")) blocks.push(<h3 key={i}>{line.slice(3)}</h3>);
    else if (line.startsWith("# ")) blocks.push(<h2 key={i}>{line.slice(2)}</h2>);
    else if (line.trim() === "") blocks.push(<div key={i} style={{ height: 6 }} />);
    else blocks.push(<p key={i}>{line}</p>);
  });
  flushList("ul-end");
  return <Fragment>{blocks}</Fragment>;
}

/** T30.1b + T31.5b: Sidebar page for local Markdown meeting notes with summary card & collapsible transcript. */
export default function NotesTab({ initialNotes }: NotesTabProps = {}) {
  const [notes, setNotes] = useState<NoteEntry[]>(initialNotes ?? []);
  const [selectedId, setSelectedId] = useState<string | null>(initialNotes?.[0]?.id ?? null);
  const [query, setQuery] = useState("");
  const [confirmDeleteId, setConfirmDeleteId] = useState<string | null>(null);
  const [showFull, setShowFull] = useState(false);
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

  const sections = useMemo(
    () => (selected ? splitNoteSections(selected.markdown) : null),
    [selected],
  );
  const durationMin = useMemo(
    () => (selected ? extractNoteDurationMin(selected.markdown) : null),
    [selected],
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
                    setShowFull(false);
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

          {selected && sections ? (
            <Group className="notes-doc">
              <div style={{ padding: "16px 20px" }}>
                <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 12, marginBottom: 14, borderBottom: "1px solid var(--border)", paddingBottom: 12 }}>
                  <div>
                    <div style={{ fontWeight: 600, fontSize: 17 }}>{selected.title}</div>
                    <div className="notes-header-meta">
                      <span>{selected.date}</span>
                      {durationMin !== null && (
                        <>
                          <span aria-hidden="true">·</span>
                          <span>{t("m31.notes_duration", { min: durationMin })}</span>
                        </>
                      )}
                    </div>
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

                {sections.summaryMd && (
                  <div className="notes-summary-card">
                    <div className="notes-summary-badge">
                      <PhIcon name="sparkle" size={14} />
                      <span>{t("m31.notes_summary")}</span>
                    </div>
                    <div className="notes-markdown">{renderMarkdownBlocks(sections.summaryMd)}</div>
                  </div>
                )}

                {sections.transcriptMd && (
                  <div style={{ marginTop: 14 }}>
                    <Button
                      aria-expanded={showFull}
                      onClick={() => setShowFull((v) => !v)}
                    >
                      <PhIcon name={showFull ? "caret-up" : "caret-down"} size={14} />
                      {showFull ? t("m31.notes_hide_full") : t("m31.notes_show_full")}
                    </Button>
                    {showFull && (
                      <div className="notes-markdown notes-transcript" style={{ marginTop: 12 }}>
                        {renderMarkdownBlocks(sections.transcriptMd)}
                      </div>
                    )}
                  </div>
                )}
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
