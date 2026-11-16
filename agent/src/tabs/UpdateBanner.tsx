import { t, type UpdateStatus } from "@decka/shared";
import { call } from "../ipc";
import Banner from "../ui/Banner";
import Button from "../ui/Button";

/** `Decka X.Y.Z is available` + `Download` (opens the release page in the browser); nothing when there is no newer release. */
export default function UpdateBanner({ status }: { status: UpdateStatus | null }) {
  const found = status?.available;
  if (!found) return null;
  return (
    <Banner kind="info">
      <span>{t("update.available", { version: found.version })}</span>
      <Button variant="primary" onClick={() => void call("open_release", { url: found.url })}>
        {t("update.download")}
      </Button>
    </Banner>
  );
}
