import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { CATALOG, setLanguage, type HaStatus, type NoteEntry } from "@decka/shared";
import NotesTab, { extractNoteDurationMin, splitNoteSections } from "../src/tabs/NotesTab";
import HaSection from "../src/tabs/HaSection";
import { loadNotes, removeNote, saveHa } from "../src/tabs/m30Ipc";
import type { call } from "../src/ipc";

const notes: NoteEntry[] = [
  { id: "2026-11-27 10-30 Синк по релизу", title: "Синк по релизу 0.60.0", date: "2026-11-27 10:30", markdown: "# Синк\n- Проверить тесты\n- Выпустить черновик" },
  { id: "2026-11-26 09-00 План", title: "План", date: "2026-11-26 09:00", markdown: "# План\nтекст" },
];

const ha: HaStatus = {
  url: "http://ha.local:8123",
  hasToken: true,
  connected: true,
  entities: [
    { id: "light.bedroom", name: "Свет в спальне", enabled: true },
    { id: "switch.desk", name: "Розетка стола", enabled: false },
  ],
};

/** A PC that records what it is asked. */
function pc(fail?: string) {
  const asked: [string, unknown][] = [];
  const c = (async (cmd: string, args?: unknown) => {
    asked.push([cmd, args]);
    if (cmd === fail) throw new Error(`${cmd} failed`);
    if (cmd === "notes_list") return notes;
    if (cmd === "ha_get") return ha;
    return undefined;
  }) as unknown as typeof call;
  return { c, asked };
}

describe("T30.1b Notes page, Home Assistant integration, and catalog", () => {
  it("opens a note: the list of the PC, the newest one shown with search, folder, delete and the sound switch", async () => {
    setLanguage("ru");
    const { c, asked } = pc();
    const list = await loadNotes(c);
    expect(asked).toEqual([["notes_list", undefined]]);
    const html = renderToStaticMarkup(<NotesTab initialNotes={list} />);
    for (const part of ["Заметки", "Открыть папку", "Синк по релизу 0.60.0", "Проверить тесты", "Удалить", "Хранить звук"]) expect(html, part).toContain(part);
  });

  it("deletes a note on the PC, and keeps the list when the PC could not", async () => {
    const { c, asked } = pc();
    const left = await removeNote(c, notes, notes[0]!.id);
    expect(asked).toEqual([["notes_delete", { id: notes[0]!.id }]]);
    expect(left.map((n) => n.title)).toEqual(["План"]);
    await expect(removeNote(pc("notes_delete").c, notes, notes[0]!.id)).rejects.toThrow("notes_delete failed");
  });

  it("saves the chosen devices of Home Assistant and nothing else; a refusal reaches the page", async () => {
    const { c, asked } = pc();
    const chosen = ha.entities.map((e) => ({ ...e, enabled: true }));
    await saveHa(c, " http://ha.local:8123 ", "", chosen);
    expect(asked).toEqual([
      ["ha_set_config", { url: "http://ha.local:8123", token: "", entities: ["light.bedroom", "switch.desk"] }],
      ["ha_get", undefined],
    ]);
    await expect(saveHa(pc("ha_set_config").c, "http://ha.local:8123", "t", [])).rejects.toThrow("ha_set_config failed");
  });

  it("renders the settings with the devices of the server and never a token", () => {
    setLanguage("ru");
    const html = renderToStaticMarkup(<HaSection initialStatus={ha} />);
    for (const part of ["Home Assistant", "Адрес Home Assistant", "Долгоживущий токен доступа", "light.bedroom", "Розетка стола", "Отключить"]) expect(html, part).toContain(part);
    expect(html).toMatch(/type="password"[^>]*value=""/);
    const empty = renderToStaticMarkup(<HaSection initialStatus={{ url: "", hasToken: false, connected: false, entities: [] }} />);
    expect(empty).not.toContain("light.bedroom");
  });

  it("includes Home Assistant and the note in the catalog as real actions", () => {
    const action = (id: string) => CATALOG.items.find((i) => i.id === id)?.action?.type;
    expect([action("ha.toggle"), action("ha.scene"), action("sound.note")]).toEqual(["ha", "ha", "notes_record"]);
  });

  it("T31.5b: puts summary card on top, collapses full transcript, and parses header duration", () => {
    setLanguage("ru");
    const longMd = [
      "Длительность: 14 мин",
      "# Кратко",
      "Договорились о сроках релиза.",
      "## Решения",
      "- Обновить дизайн",
      "## Расшифровка",
      "Полная стенограмма разговора.",
    ].join("\n");
    expect(extractNoteDurationMin(longMd)).toBe(14);
    const split = splitNoteSections(longMd);
    expect(split.summaryMd).toContain("# Кратко");
    expect(split.summaryMd).toContain("Обновить дизайн");
    expect(split.transcriptMd).toContain("Полная стенограмма разговора.");

    const html = renderToStaticMarkup(
      <NotesTab initialNotes={[{ id: "long", title: "Большой созвон", date: "2026-11-27 11:00", markdown: longMd }]} />,
    );
    expect(html).toContain("notes-summary-card");
    expect(html).toContain("14 мин");
    expect(html).toContain("Показать полный текст");
    expect(html).not.toContain("Полная стенограмма разговора.");
  });
});
