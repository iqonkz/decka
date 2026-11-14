import type { ActionsFile } from "./actions";
import type { LayoutFile, LayoutItem } from "./layout";
import type { ValidationIssue } from "./ipc";

const ID_RE = /^[a-z0-9_-]{1,64}$/;

// Allowed sizes per kind: [minW, maxW, minH, maxH]
const SIZES: Record<string, [number, number, number, number]> = {
  button: [1, 1, 1, 1],
  clock: [1, 2, 1, 1],
  sys: [2, 2, 1, 2],
  status: [1, 1, 1, 1],
  statuses: [2, 5, 1, 3],
  notifications: [2, 5, 1, 3],
};

const esc = (s: string): string => s.replace(/~/g, "~0").replace(/\//g, "~1");

function overlaps(a: LayoutItem, b: LayoutItem): boolean {
  return a.x < b.x + b.w && b.x < a.x + a.w && a.y < b.y + b.h && b.y < a.y + a.h;
}

// Semantic checks from DATA_MODEL.md 3.2 (items 2-5); structural checks live in the JSON Schema.
export function validateLayout(layout: LayoutFile, actions: ActionsFile): ValidationIssue[] {
  const issues: ValidationIssue[] = [];
  const add = (path: string, message: string) => issues.push({ path, message });

  const pageIds = new Set<string>();
  layout.pages.forEach((page, pi) => {
    if (pageIds.has(page.id)) add(`/pages/${pi}`, `duplicate page id '${page.id}'`);
    pageIds.add(page.id);
  });

  const itemIds = new Set<string>();
  layout.pages.forEach((page, pi) => {
    page.items.forEach((item, ii) => {
      const path = `/pages/${pi}/items/${ii}`;

      if (itemIds.has(item.id)) add(path, `duplicate item id '${item.id}'`);
      itemIds.add(item.id);

      if (item.x < 0 || item.y < 0 || item.w < 1 || item.h < 1 || item.x + item.w > 5 || item.y + item.h > 3) {
        add(path, "item is outside the 5x3 grid");
      }

      for (let j = 0; j < ii; j++) {
        const other = page.items[j];
        if (other && overlaps(item, other)) {
          add(path, `item overlaps '${other.id}'`);
          break;
        }
      }

      const size = SIZES[item.kind];
      if (size && (item.w < size[0] || item.w > size[1] || item.h < size[2] || item.h > size[3])) {
        add(path, `size ${item.w}x${item.h} is not allowed for kind '${item.kind}'`);
      }

      if (item.kind === "button") {
        const hasAction = item.actionId !== undefined;
        const hasNav = item.navigate !== undefined;
        if (hasAction === hasNav) {
          add(path, "button needs exactly one of actionId / navigate");
        }
        if (item.actionId !== undefined && !Object.hasOwn(actions.actions, item.actionId)) {
          add(path, `unknown actionId '${item.actionId}'`);
        }
        if (item.navigate !== undefined && !pageIds.has(item.navigate)) {
          add(path, `unknown page '${item.navigate}'`);
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
