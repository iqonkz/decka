import type { LayoutFile, ValidationIssue } from "@decka/shared";

/** A readable form of a validation issue: which page and element, then what to fix. */
export function describeIssue(issue: ValidationIssue, layout: LayoutFile): string {
  const item = /^\/pages\/(\d+)\/items\/(\d+)/.exec(issue.path);
  if (item) {
    const page = layout.pages[Number(item[1])];
    const it = page?.items[Number(item[2])];
    if (page && it) return `Page "${page.name}", ${it.kind} at column ${it.x + 1}, row ${it.y + 1}: ${issue.message}`;
  }
  const apps = /^\/pages\/(\d+)\/apps/.exec(issue.path);
  if (apps) {
    const page = layout.pages[Number(apps[1])];
    if (page) return `Page "${page.name}": ${issue.message}`;
  }
  if (issue.path === "/grid" || issue.path.startsWith("/grid/")) return `Grid: ${issue.message}`;
  return `${issue.path || "/"}: ${issue.message}`;
}

/** "/pages/1/items/3" -> [1, 3]; anything else -> null. */
export function itemPath(path: string): [number, number] | null {
  const m = /^\/pages\/(\d+)\/items\/(\d+)/.exec(path);
  return m ? [Number(m[1]), Number(m[2])] : null;
}
