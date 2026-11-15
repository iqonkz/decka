import type { InstalledApp } from "@decka/shared";
import { call } from "../ipc";

/** The Start menu programs are read once per settings-window session (it takes seconds); `reloadApps` reads them again. */
let cache: InstalledApp[] | null = null;
let pending: Promise<InstalledApp[]> | null = null;

export const cachedApps = (): InstalledApp[] | null => cache;

export function loadApps(force = false): Promise<InstalledApp[]> {
  if (cache && !force) return Promise.resolve(cache);
  pending ??= call("list_installed_apps")
    .then((list) => (cache = list))
    .finally(() => {
      pending = null;
    });
  return pending;
}