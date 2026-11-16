import { useCallback, useEffect, useState } from "react";
import type { UpdateStatus } from "@decka/shared";
import { listen } from "@tauri-apps/api/event";
import { call } from "./ipc";

/** The page of the releases of the project (the only place `open_release` opens besides a release's own page). */
export const RELEASES_URL = "https://github.com/iqonkz/decka/releases/";

/** The release page of one version: its notes are the CHANGELOG of that release. */
export const releasePageOf = (version: string): string => `${RELEASES_URL}tag/v${version}`;

/**
 * The answer of the last update check, kept up to date (the agent checks once a day and on `Check now`). `check` asks
 * GitHub at once; the result also arrives through the `decka://update-changed` event.
 */
export function useUpdates(): { status: UpdateStatus | null; check: () => Promise<void>; checking: boolean } {
  const [status, setStatus] = useState<UpdateStatus | null>(null);
  const [checking, setChecking] = useState(false);

  useEffect(() => {
    let off: (() => void) | null = null;
    let disposed = false;
    call("get_update_status").then((s) => !disposed && setStatus(s), () => {});
    listen<UpdateStatus>("decka://update-changed", (event) => setStatus(event.payload)).then((unlisten) => {
      if (disposed) unlisten();
      else off = unlisten;
    });
    return () => {
      disposed = true;
      off?.();
    };
  }, []);

  const check = useCallback(async () => {
    setChecking(true);
    try {
      setStatus(await call("check_updates"));
    } catch {
      // the command itself failed (no agent): leave the status as it is
    } finally {
      setChecking(false);
    }
  }, []);

  return { status, check, checking };
}
