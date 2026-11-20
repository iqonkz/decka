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
  names?: Record<string, string>;      // M19: the name in the other languages by code (`name` is English)
  actionNames?: Record<string, Record<Id, string>>;   // M19: the captions of the actions by language code and action id (the app strip)
}
