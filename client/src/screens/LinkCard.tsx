import { useEffect, useState } from "react";
import { t } from "@decka/shared";
import { connectAgain, retryConnection, unpair } from "../app/controller";
import { cardButtons, linkView, type LinkView } from "../net/linkState";
import { useAppState } from "../store/store";

/** The view of the link: the card appears 8 s after the link was lost (at once when the PC rejected the key). */
export function useLinkView(): LinkView {
  const conn = useAppState((s) => s.conn);
  const [since, setSince] = useState<number | null>(null);
  const [now, setNow] = useState(0);
  // The clock of "no link" starts when the link goes away and is reset by the link coming back.
  useEffect(() => {
    if (conn === "connecting" || conn === "offline") setSince((s) => s ?? Date.now());
    else setSince(null);
  }, [conn]);
  useEffect(() => {
    if (since === null) return;
    const id = window.setInterval(() => setNow(Date.now()), 500);
    return () => window.clearInterval(id);
  }, [since]);
  return linkView(conn, since === null ? 0 : Math.max(0, now - since));
}

/** "Can't reach {pc}" over the dimmed layout: the way out is always here (DESIGN.md 4.13). */
export default function LinkCard({ reason }: { reason: "silent" | "rejected" }) {
  const pc = useAppState((s) => s.agentName) ?? "PC";
  const [why, setWhy] = useState(false);
  const buttons = cardButtons(reason);
  return (
    <div className="link-backdrop">
      <div className="link-card" role="dialog" aria-label={t("link.title", { pc })}>
        <h2>{t("link.title", { pc })}</h2>
        <p>{reason === "silent" ? t("link.silent") : t("link.rejected")}</p>
        <div className="link-buttons">
          {buttons.tryAgain && (
            <button className="primary" type="button" onClick={retryConnection}>
              {t("link.try_again")}
            </button>
          )}
          <button className={buttons.tryAgain ? "" : "primary"} type="button" onClick={connectAgain}>
            {t("link.connect_again")}
          </button>
        </div>
        <button type="button" className="link-small" onClick={unpair}>
          {t("link.other_pc")}
        </button>
        <button type="button" className="link-small" aria-expanded={why} onClick={() => setWhy(!why)}>
          {t("link.why")}
        </button>
        {why && (
          <ul className="connecting-hints">
            <li>{t("phone.hint_wifi")}</li>
            <li>{t("phone.hint_agent")}</li>
            <li>{t("link.hint_public")}</li>
            <li>{t("link.hint_usb")}</li>
          </ul>
        )}
      </div>
    </div>
  );
}
