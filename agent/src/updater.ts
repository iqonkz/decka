import { check } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";

/** Where the one-click update of the agent is (T20.3). */
export type UpdatePhase =
  | { phase: "idle" }
  | { phase: "downloading"; pct: number }
  | { phase: "installing" }
  | { phase: "error"; kind: "signature" | "other" };

/** Percent of the download, 0..100; 0 while the size is unknown. */
export function downloadPct(downloaded: number, total: number | undefined): number {
  if (!total || total <= 0) return 0;
  return Math.min(100, Math.max(0, Math.floor((downloaded / total) * 100)));
}

/** A failed signature check is told apart from other failures: nothing was installed either way. */
export function classifyUpdateError(error: unknown): "signature" | "other" {
  const text = error instanceof Error ? error.message : String(error);
  return /signature|minisign|pubkey|public key/i.test(text) ? "signature" : "other";
}

/**
 * Checks the release endpoint (`plugins.updater` in tauri.conf.json), downloads the installer, lets the plugin verify its
 * signature against the public key built into the agent, installs it (NSIS, passive) and restarts the agent.
 * Throws when nothing newer is found; `onPhase` gets the progress.
 */
export async function installUpdate(onPhase: (p: UpdatePhase) => void): Promise<void> {
  try {
    const update = await check();
    if (!update) throw new Error("no update");
    let total: number | undefined;
    let downloaded = 0;
    onPhase({ phase: "downloading", pct: 0 });
    await update.downloadAndInstall((event) => {
      if (event.event === "Started") total = event.data.contentLength;
      else if (event.event === "Progress") {
        downloaded += event.data.chunkLength;
        onPhase({ phase: "downloading", pct: downloadPct(downloaded, total) });
      } else if (event.event === "Finished") onPhase({ phase: "installing" });
    });
    onPhase({ phase: "installing" });
    await relaunch();
  } catch (error) {
    onPhase({ phase: "error", kind: classifyUpdateError(error) });
  }
}
