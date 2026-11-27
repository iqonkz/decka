import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { CATALOG, setLanguage } from "@decka/shared";
import NotesTab from "../src/tabs/NotesTab";
import HaSection from "../src/tabs/HaSection";

describe("T30.1b Notes page, Home Assistant integration, and catalog", () => {
  it("renders NotesTab with search, Open folder, markdown viewer, and Delete button", () => {
    setLanguage("ru");
    const html = renderToStaticMarkup(
      <NotesTab
        initialNotes={[
          {
            id: "n1",
            title: "Синк по релизу 0.60.0",
            date: "2026-11-27 10:30",
            markdown: "# Сводка\n- Проверить тесты\n- Выпустить черновик",
          },
        ]}
      />,
    );
    expect(html).toContain("Заметки");
    expect(html).toContain("Открыть папку");
    expect(html).toContain("Синк по релизу 0.60.0");
    expect(html).toContain("Проверить тесты");
    expect(html).toContain("Удалить");
  });

  it("renders HaSection with URL, token, and entity checkboxes", () => {
    setLanguage("ru");
    const html = renderToStaticMarkup(
      <HaSection initialUrl="http://ha.local:8123" initialToken="secret" />,
    );
    expect(html).toContain("Home Assistant");
    expect(html).toContain("Адрес Home Assistant");
    expect(html).toContain("Долгоживущий токен доступа");
    expect(html).toContain("light.bedroom");
    expect(html).toContain("switch.desk");
  });

  it("includes Home Assistant items in the catalog", () => {
    const toggle = CATALOG.items.find((i) => i.id === "ha.toggle");
    const scene = CATALOG.items.find((i) => i.id === "ha.scene");
    expect(toggle).toBeTruthy();
    expect(scene).toBeTruthy();
  });
});
