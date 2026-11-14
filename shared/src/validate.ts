import type { ActionsFile } from "./actions";
import type { LayoutFile, LayoutItem } from "./layout";
import type { ValidationIssue } from "./ipc";

const ID_RE = /^[a-z0-9_-]{1,64}$/;

// Allowed sizes per kind: [minW, maxW, minH, maxH]; the two list widgets may fill the whole grid.
function sizeLimits(kind: string, grid: LayoutFile["grid"]): [number, number, number, number] | undefined {
  switch (kind) {
    case "button":
    case "status":
      return [1, 1, 1, 1];
    case "clock":
      return [1, 2, 1, 1];
    case "sys":
      return [2, 2, 1, 2];
    case "statuses":
    case "notifications":
      return [2, grid.cols, 1, grid.rows];
    case "media":
      return [2, grid.cols, 1, 2];
    default:
      return undefined;
  }
}

const esc = (s: string): string => s.replace(/~/g, "~0").replace(/\//g, "~1");

function overlaps(a: LayoutItem, b: LayoutItem): boolean {
  return a.x < b.x + b.w && b.x < a.x + a.w && a.y < b.y + b.h && b.y < a.y + a.h;
}

// Semantic checks from DATA_MODEL.md 3.2 (items 2-5); structural checks live in the JSON Schema.
export function validateLayout(layout: LayoutFile, actions: ActionsFile): ValidationIssue[] {
  const issues: ValidationIssue[] = [];
  const add = (path: string, message: string) => issues.push({ path, message });

  const { cols, rows } = layout.grid;
  if (!Number.isInteger(cols) || cols < 3 || cols > 8) add("/grid/cols", "grid columns must be an integer from 3 to 8");
  if (!Number.isInteger(rows) || rows < 2 || rows > 5) add("/grid/rows", "grid rows must be an integer from 2 to 5");

  const pageIds = new Set<string>();
  layout.pages.forEach((page, pi) => {
    if (pageIds.has(page.id)) add(`/pages/${pi}`, `duplicate page id '${page.id}'`);
    pageIds.add(page.id);
  });

  // Context pages (DATA_MODEL.md 3.2 item 9): the first page is home; an app belongs to one page only.
  const appOwner = new Map<string, string>();
  layout.pages.forEach((page, pi) => {
    if (page.apps === undefined) return;
    if (pi === 0 && page.apps.length > 0) add("/pages/0/apps", "the first page is the home page and cannot have apps");
    page.apps.forEach((app, ai) => {
      const owner = appOwner.get(app);
      if (owner !== undefined && owner !== page.id) add(`/pages/${pi}/apps/${ai}`, `${app} is already used by another page`);
      else if (owner === page.id) add(`/pages/${pi}/apps/${ai}`, `duplicate app '${app}'`);
      else appOwner.set(app, page.id);
    });
  });

  const itemIds = new Set<string>();
  layout.pages.forEach((page, pi) => {
    page.items.forEach((item, ii) => {
      const path = `/pages/${pi}/items/${ii}`;

      if (itemIds.has(item.id)) add(path, `duplicate item id '${item.id}'`);
      itemIds.add(item.id);

      if (item.x < 0 || item.y < 0 || item.w < 1 || item.h < 1 || item.x + item.w > cols || item.y + item.h > rows) {
        add(path, `item is outside the ${cols}x${rows} grid`);
      }

      for (let j = 0; j < ii; j++) {
        const other = page.items[j];
        if (other && overlaps(item, other)) {
          add(path, `item overlaps '${other.id}'`);
          break;
        }
      }

      const size = sizeLimits(item.kind, layout.grid);
      if (size && (item.w < size[0] || item.w > size[1] || item.h < size[2] || item.h > size[3])) {
        add(path, `size ${item.w}x${item.h} is not allowed for kind '${item.kind}'`);
      }

      if (item.kind === "media") {
        for (const [field, id] of [["prevAction", item.prevAction], ["playAction", item.playAction], ["nextAction", item.nextAction]] as const) {
          if (id !== undefined && !Object.hasOwn(actions.actions, id)) add(path, `unknown ${field} '${id}'`);
        }
      }

      if (item.kind === "button") {
        const hasAction = item.actionId !== undefined;
        const hasNav = item.navigate !== undefined;
        if (!hasAction && !hasNav) {
          add(path, "choose an action or a page for this button");
        } else if (hasAction && hasNav) {
          add(path, "a button can either run an action or open a page, not both");
        }
        if (item.actionId !== undefined && !Object.hasOwn(actions.actions, item.actionId)) {
          add(path, `action "${item.actionId}" no longer exists — choose another`);
        }
        if (item.navigate !== undefined && !pageIds.has(item.navigate)) {
          add(path, `page "${item.navigate}" no longer exists — choose another`);
        }
        if (item.confirm !== undefined && !hasAction) {
          add(path, "confirm is only allowed with actionId");
        }
      }
    });
  });

  return issues;
}

export function validateActions(actions: ActionsFile): ValidationIssue[] {
  const issues: ValidationIssue[] = [];
  for (const [id, action] of Object.entries(actions.actions)) {
    if (!ID_RE.test(id)) issues.push({ path: `/actions/${esc(id)}`, message: `invalid action id '${id}'` });
    if (action.type === "hotkey") {
      const seen = new Set<string>();
      action.keys.forEach((key, ki) => {
        if (seen.has(key)) issues.push({ path: `/actions/${esc(id)}/keys/${ki}`, message: `duplicate key '${key}'` });
        seen.add(key);
      });
    }
  }
  return issues;
}
