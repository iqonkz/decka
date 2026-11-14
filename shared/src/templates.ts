import type { Action } from "./actions";
import type { LayoutItem } from "./layout";
import type { Id } from "./protocol";

/** A ready-made context page (DATA_MODEL.md 7). Files live in shared/templates/<id>.json. */
export interface PageTemplate {
  id: Id;                              // also the preferred page id
  name: string;                        // page name
  apps: string[];                      // context apps
  minGrid: { cols: number; rows: number };
  actions: Record<Id, Action>;         // action ids start with "tpl-<id>-"
  items: LayoutItem[];                 // navigate: "$home" is replaced by pages[0].id
}
