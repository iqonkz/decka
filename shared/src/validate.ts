import { isReservedVar, SEQUENCE_LIMITS, type ActionsFile } from "./actions";
import { BACK_PAGE, type LayoutFile, type LayoutItem, type Orientation } from "./layout";
import type { ValidationIssue } from "./ipc";

const ID_RE = /^[a-z0-9_-]{1,64}$/;

/** The grid limits of an orientation (DATA_MODEL.md 3.2 rule 1); `undefined` is landscape. */
export function gridLimits(orientation: Orientation | undefined): { orientation: Orientation; cols: [number, number]; rows: [number, number] } {
  return orientation === "portrait" ? { orientation: "portrait", cols: [2, 5], rows: [3, 8] } : { orientation: "landscape", cols: [3, 8], rows: [2, 5] };
}

/** Whether `cols` x `rows` are whole numbers within the limits of the orientation. */
export function gridFits(cols: number, rows: number, orientation: Orientation | undefined): boolean {
  const l = gridLimits(orientation);
  return Number.isInteger(cols) && Number.isInteger(rows) && cols >= l.cols[0] && cols <= l.cols[1] && rows >= l.rows[0] && rows <= l.rows[1];
}

// Allowed sizes per kind as a box: [minW, maxW, minH, maxH]; the two list widgets may fill the whole grid. For the kinds whose
// sizes are a list (volume, timer, weather) this is the bounding box: `sizeAllowed` is the exact rule (DATA_MODEL.md 3.2 rule 13).
export function sizeLimits(kind: string, grid: LayoutFile["grid"], graph = false): [number, number, number, number] | undefined {
  switch (kind) {
    case "button":
    case "status":
      return [1, 1, 1, 1];
    case "clock":
      return [1, 2, 1, 1];
    case "sys":
      return graph ? [2, 4, 1, 2] : [2, 2, 1, 2];
    case "statuses":
    case "notifications":
      return [2, grid.cols, 1, grid.rows];
    case "media":
      return [2, grid.cols, 1, 2];
    case "volume":
      return [1, 4, 1, 4];
    case "timer":
      return [1, 2, 1, 2];
    case "weather":
      return [1, 4, 1, 2];
    case "obs":
    case "ai":
      return [2, 4, 1, 2];
    default:
      return undefined;
  }
}

/** Whether `w` x `h` is an allowed size of the kind (the box of `sizeLimits` plus the M15 lists). */
export function sizeAllowed(kind: string, w: number, h: number, grid: LayoutFile["grid"], graph = false): boolean {
  const box = sizeLimits(kind, grid, graph);
  if (!box) return true;
  if (w < box[0] || w > box[1] || h < box[2] || h > box[3]) return false;
  switch (kind) {
    case "volume":
      return (w === 1 && h >= 2) || (h === 1 && w >= 2); // 1x2..1x4 vertical or 2x1..4x1 horizontal (1x1 is neither)
    case "timer":
      return !(w === 1 && h === 2); // 1x1, 2x1, 2x2
    case "weather":
      return h === 1 ? w <= 2 : w >= 3; // 1x1, 2x1, 3x2, 4x2
    case "ai":
      return h === 2 || w === 2; // 2x1, 2x2, 3x2, 4x2
    default:
      return true;
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
  // The limits depend on the orientation (DATA_MODEL.md 3.2 rule 1): landscape 3..8 x 2..5, portrait 2..5 x 3..8.
  const limits = gridLimits(layout.display?.orientation);
  if (!gridFits(cols, rows, layout.display?.orientation)) {
    const [name, [cMin, cMax], [rMin, rMax]] = [limits.orientation, limits.cols, limits.rows];
    add("/grid", `Grid ${cols}×${rows} does not fit ${name} (${cMin}–${cMax} columns, ${rMin}–${rMax} rows)`);
  }
  const orientation: unknown = layout.display?.orientation;
  if (orientation !== undefined && orientation !== "landscape" && orientation !== "portrait") add("/display/orientation", "orientation must be landscape or portrait");

  if (layout.display?.keyFace !== undefined && layout.display.keyFace !== "standard" && layout.display.keyFace !== "full") add("/display/keyFace", "keyFace must be standard or full");
  const language: unknown = layout.display?.language;
  if (language !== undefined && language !== "auto" && language !== "en" && language !== "ru") add("/display/language", "language must be auto, en or ru");

  const pageIds = new Set<string>();
  const popupIds = new Set<string>();
  layout.pages.forEach((page, pi) => {
    if (pageIds.has(page.id)) add(`/pages/${pi}`, `duplicate page id '${page.id}'`);
    pageIds.add(page.id);
    if (page.popup === true) popupIds.add(page.id);
    // Popup groups (DATA_MODEL.md 3.2 rule 12)
    if (page.popup === true && pi === 0) add("/pages/0/popup", "the first page cannot be a popup group");
    if (page.popup === true && page.apps !== undefined && page.apps.length > 0) add(`/pages/${pi}/apps`, "a popup group cannot show itself for apps: remove them");
    if (page.stayOpen !== undefined && page.popup !== true) add(`/pages/${pi}/stayOpen`, "stayOpen only makes sense for a popup group");
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

      if (!sizeAllowed(item.kind, item.w, item.h, layout.grid, item.kind === "sys" && item.graph === true)) {
        add(path, `size ${item.w}x${item.h} is not allowed for kind '${item.kind}'`);
      }

      if (item.kind === "media") {
        for (const [field, id] of [["prevAction", item.prevAction], ["playAction", item.playAction], ["nextAction", item.nextAction]] as const) {
          if (id !== undefined && !Object.hasOwn(actions.actions, id)) add(path, `unknown ${field} '${id}'`);
        }
      }
      if (item.kind === "volume") {
        for (const [field, id] of [["upAction", item.upAction], ["downAction", item.downAction], ["muteAction", item.muteAction]] as const) {
          if (id !== undefined && !Object.hasOwn(actions.actions, id)) add(path, `unknown ${field} '${id}'`);
        }
      }
      if (item.kind === "timer" && item.presets !== undefined) {
        const p = item.presets;
        if (!Array.isArray(p) || p.length < 1 || p.length > 4 || p.some((m) => !Number.isInteger(m) || m < 1 || m > 180)) {
          add(`${path}/presets`, "presets must be 1..4 whole numbers of minutes, 1..180");
        }
      }

      if (item.kind === "button") {
        if (item.face !== undefined && item.face !== "standard" && item.face !== "full") add(`${path}/face`, "face must be standard or full");
        const hasAction = item.actionId !== undefined;
        const hasNav = item.navigate !== undefined;
        const hasPopup = item.popup !== undefined;
        // Allowed: actionId | navigate | popup | actionId + popup.
        if (!hasAction && !hasNav && !hasPopup) {
          add(path, "choose an action or a page for this button");
        } else if (hasNav && hasAction) {
          add(path, "a button can either run an action or open a page, not both");
        } else if (hasNav && hasPopup) {
          add(path, "a button can open a page or a popup group, not both");
        }
        if (item.actionId !== undefined && !Object.hasOwn(actions.actions, item.actionId)) {
          add(path, `action "${item.actionId}" no longer exists — choose another`);
        }
        // "$back" (DATA_MODEL.md 3.8) names no page; inside a popup group it is refused below like any `navigate`.
        if (item.navigate !== undefined && item.navigate !== BACK_PAGE) {
          if (!pageIds.has(item.navigate)) add(path, `page "${item.navigate}" no longer exists — choose another`);
          else if (popupIds.has(item.navigate)) add(path, `page "${item.navigate}" is a popup group — choose a normal page`);
        }
        if (item.popup !== undefined) {
          if (!pageIds.has(item.popup)) add(path, `popup group "${item.popup}" no longer exists — choose another`);
          else if (!popupIds.has(item.popup)) add(path, `page "${item.popup}" is not a popup group`);
        }
        if (page.popup === true && (hasNav || hasPopup)) {
          add(path, "a key inside a popup group cannot open a page or another group");
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
    const base = `/actions/${esc(id)}`;
    if (action.toggleVar !== undefined && isReservedVar(action.toggleVar)) {
      issues.push({ path: `${base}/toggleVar`, message: `toggleVar: "${action.toggleVar}" is set by Decka itself — remove it` });
    }
    if (action.type === "hotkey") {
      const seen = new Set<string>();
      action.keys.forEach((key, ki) => {
        if (seen.has(key)) issues.push({ path: `${base}/keys/${ki}`, message: `duplicate key '${key}'` });
        seen.add(key);
      });
    }
    if (action.type === "obs") {
      // DATA_MODEL.md 2.3: `scene` goes with op "scene" and `input` with "input_mute_toggle", and with nothing else.
      const need = (field: "scene" | "input", op: string) => {
        const value = action[field];
        if (action.op === op) {
          if (typeof value !== "string" || value.length < 1 || value.length > 100) issues.push({ path: `${base}/${field}`, message: `${field} is required (1..100 characters) for ${op}` });
        } else if (value !== undefined) {
          issues.push({ path: `${base}/${field}`, message: `${field} is only allowed with ${op}` });
        }
      };
      need("scene", "scene");
      need("input", "input_mute_toggle");
    }
    if (action.type === "sequence") {
      if (action.steps.length < 1 || action.steps.length > SEQUENCE_LIMITS.steps) issues.push({ path: `${base}/steps`, message: `a sequence has 1..${SEQUENCE_LIMITS.steps} steps` });
      let delays = 0;
      action.steps.forEach((step, si) => {
        const at = `${base}/steps/${si}`;
        if ("delayMs" in step) {
          if (!Number.isInteger(step.delayMs) || step.delayMs < SEQUENCE_LIMITS.delayMin || step.delayMs > SEQUENCE_LIMITS.delayMax) {
            issues.push({ path: at, message: `a pause is ${SEQUENCE_LIMITS.delayMin}..${SEQUENCE_LIMITS.delayMax} ms` });
          } else {
            delays += step.delayMs;
          }
        } else if (!Object.hasOwn(actions.actions, step.actionId)) {
          issues.push({ path: at, message: `action "${step.actionId}" does not exist` });
        } else if (actions.actions[step.actionId]!.type === "sequence") {
          issues.push({ path: at, message: "a sequence cannot contain another sequence" });
        }
      });
      if (delays > SEQUENCE_LIMITS.delaySum) issues.push({ path: `${base}/steps`, message: `the pauses add up to more than ${SEQUENCE_LIMITS.delaySum} ms` });
    }
  }
  return issues;
}
