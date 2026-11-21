import { useState } from "react";
import { t, type UpdateStatus } from "@decka/shared";
import { call } from "../ipc";
import { installUpdate, type UpdatePhase } from "../updater";
import Banner from "../ui/Banner";
import Button from "../ui/Button";

/**
 * `Decka X.Y.Z is available` + `Update to X.Y.Z` (downloads, verifies the signature, installs, restarts) + `Download`
 * (opens the release page: the fallback); nothing when there is no newer release.
 */
export default function UpdateBanner({ status }: { status: UpdateStatus | null }) {
  const [phase, setPhase] = useState<UpdatePhase>({ phase: "idle" });
  const found = status?.available;
  if (!found) return null;
  const busy = phase.phase === "downloading" || phase.phase === "installing";
  let line: string | null = null;
  if (phase.phase === "downloading") line = t("update.downloading", { pct: phase.pct });
  else if (phase.phase === "installing") line = t("update.installing");
  else if (phase.phase === "error") line = phase.kind === "signature" ? t("update.not_signed") : t("update.failed");
  return (
    <Banner kind={phase.phase === "error" ? "error" : "info"}>
      <span>{line ?? t("update.available", { version: found.version })}</span>
      {!busy && (
        <>
          <Button variant="primary" onClick={() => void installUpdate(setPhase)}>
            {t("update.install", { version: found.version })}
          </Button>
          <Button onClick={() => void call("open_release", { url: found.url })}>{t("update.download")}</Button>
        </>
      )}
    </Banner>
  );
}
