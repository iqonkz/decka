// T29.6: the voice asked for a catalog key that needs something typed. The PC opens this window at `keys:catalog:<id>`;
// the editor takes the id once it is ready and opens the same dialog as a click in the library.

export const CATALOG_EVENT = "decka:catalog-item";
const TARGET = /^keys:catalog:([a-z0-9._-]{1,64})$/;

let pending: string | null = null;

/** Remembers the item of a navigation target; `false` when the target is something else. */
export function requestCatalogItem(target: string): boolean {
  const id = TARGET.exec(target)?.[1];
  if (id === undefined) return false;
  pending = id;
  window.dispatchEvent(new Event(CATALOG_EVENT));
  return true;
}

export function takeCatalogItem(): string | null {
  const id = pending;
  pending = null;
  return id;
}
