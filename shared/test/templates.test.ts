import Ajv2020 from "ajv/dist/2020";
import { describe, expect, test } from "vitest";
import actionsSchema from "../schemas/actions.schema.json";
import layoutSchema from "../schemas/layout.schema.json";
import layoutDefault from "../fixtures/layout.default.json";
import vscode from "../templates/vscode.json";
import browser from "../templates/browser.json";
import zoom from "../templates/zoom.json";
import spotify from "../templates/spotify.json";
import outlook from "../templates/outlook.json";
import word from "../templates/word.json";
import excel from "../templates/excel.json";
import powerpoint from "../templates/powerpoint.json";
import canva from "../templates/canva.json";
import telegram from "../templates/telegram.json";
import whatsapp from "../templates/whatsapp.json";
import discord from "../templates/discord.json";
import type { LayoutFile, LayoutItem } from "../src/layout";
import type { PageTemplate } from "../src/templates";
import { validateLayout } from "../src/validate";

const templates = [vscode, browser, zoom, spotify, outlook, word, excel, powerpoint, canva, telegram, whatsapp, discord] as unknown as PageTemplate[];

const ajv = new Ajv2020({ allErrors: true, strict: false });
ajv.addSchema(actionsSchema);
ajv.addSchema(layoutSchema);
const ID = "https://decka.invalid/schemas";
const action = ajv.getSchema(`${ID}/actions.schema.json#/$defs/Action`)!;
const layoutFile = ajv.getSchema(`${ID}/layout.schema.json#/$defs/LayoutFile`)!;

/** What `add_template` builds on an otherwise empty layout with the smallest grid the template accepts. */
function withTemplate(t: PageTemplate): LayoutFile {
  const display = (layoutDefault as unknown as LayoutFile).display;
  const items = t.items.map((item) => ("navigate" in item && item.navigate === "$home" ? { ...item, navigate: "main" } : item)) as LayoutItem[];
  return {
    version: 1,
    grid: t.minGrid,
    display,
    pages: [
      { id: "main", name: "Main", items: [] },
      { id: t.id, name: t.name, apps: t.apps, items },
    ],
  };
}

describe.each(templates.map((t) => [t.id, t] as const))("template %s", (_id, t) => {
  test("every action is valid and its id starts with tpl-<id>-", () => {
    for (const [id, a] of Object.entries(t.actions)) {
      expect(action(a), `${id}: ${JSON.stringify(action.errors)}`).toBe(true);
      expect(id.startsWith(`tpl-${t.id}-`), id).toBe(true);
    }
  });

  test("the page built from it is a valid layout with 0 issues", () => {
    const layout = withTemplate(t);
    expect(layoutFile(layout), JSON.stringify(layoutFile.errors)).toBe(true);
    expect(validateLayout(layout, { version: 1, actions: t.actions })).toEqual([]);
  });

  test("the minimal grid is 7x4", () => {
    expect(t.minGrid).toEqual({ cols: 7, rows: 4 });
  });
});
