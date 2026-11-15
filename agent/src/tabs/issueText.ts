import { t, type LayoutFile, type ValidationIssue } from "@decka/shared";

/** A readable form of a validation issue: which page and element, then what to fix. (The message itself comes from the PC in English.) */
export function describeIssue(issue: ValidationIssue, layout: LayoutFile): string {
  const item = /^\/pages\/(\d+)\/items\/(\d+)/.exec(issue.path);
  if (item) {
    const page = layout.pages[Number(item[1])];
    const it = page?.items[Number(item[2])];
    if (page && it) return t("issue.item", { page: page.name, kind: t(`kind.${it.kind}`), col: it.x + 1, row: it.y + 1, message: issue.message });
  }
  const apps = /^\/pages\/(\d+)\/apps/.exec(issue.path);
  if (apps) {
    const page = layout.pages[Number(apps[1])];
    if (page) return t("issue.page", { page: page.name, message: issue.message });
  }
  if (issue.path === "/grid" || issue.path.startsWith("/grid/")) return t("issue.grid", { message: issue.message });
  return `${issue.path || "/"}: ${issue.message}`;
}

/** "/pages/1/items/3" -> [1, 3]; anything else -> null. */
export function itemPath(path: string): [number, number] | null {
  const m = /^\/pages\/(\d+)\/items\/(\d+)/.exec(path);
  return m ? [Number(m[1]), Number(m[2])] : null;
}
