import { useEffect, useState } from "react";
import { t } from "@decka/shared";
import { connectAgain, retryConnection, switchPc } from "../app/controller";
import { cardButtons, linkView, type LinkView } from "../net/linkState";
import { usePcs } from "../net/pcs";
import { isNative } from "../platform/native";
import { useAppState } from "../store/store";
import ServiceCard from "../ui/ServiceCard";

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

/** "Can't reach {pc}" over the dimmed layout: the way out is always here (DESIGN.md 4.13, 4.16). */
export default function LinkCard({ reason }: { reason: "silent" | "rejected" }) {
  const pc = useAppState((s) => s.agentName) ?? "PC";
  const [why, setWhy] = useState(false);
  const buttons = cardButtons(reason);
  // T32.4: this PC does not answer - offer the paired ones that do. The web page belongs to one PC only.
  const others = usePcs(isNative(), false).filter((p) => !p.current && p.online);
  return (
    <ServiceCard
      backdrop
      role="dialog"
      icon="wifi-slash"
      title={t("link.title", { pc })}
      text={reason === "silent" ? t("link.silent") : t("link.rejected")}
    >
      <div className="service-status-row">
        <span className={reason === "silent" ? "service-pulse-dot warn" : "service-pulse-dot err"} aria-hidden="true" />
        <span>{reason === "silent" ? t("phone.connecting") : t("link.rejected")}</span>
      </div>
      <div className="service-actions">
        {others.map((p) => (
          <button key={p.id} className="pill-btn primary" type="button" onClick={() => switchPc(p.id)}>
            {t("pcs.switch_to", { name: p.name })}
          </button>
        ))}
        {buttons.tryAgain && (
          <button className="pill-btn primary" type="button" onClick={retryConnection}>
            {t("link.try_again")}
          </button>
        )}
        <button className={buttons.tryAgain ? "pill-btn secondary" : "pill-btn primary"} type="button" onClick={connectAgain}>
          {t("link.connect_again")}
        </button>
        <button type="button" className="pill-btn secondary" onClick={connectAgain}>
          {t("link.other_pc")}
        </button>
        <button type="button" className="link-small" aria-expanded={why} onClick={() => setWhy(!why)}>
          {t("link.why")}
        </button>
      </div>
      {why && (
        <ul className="connecting-hints">
          <li>{t("phone.hint_wifi")}</li>
          <li>{t("phone.hint_agent")}</li>
          <li>{t("link.hint_public")}</li>
          <li>{t("link.hint_usb")}</li>
        </ul>
      )}
    </ServiceCard>
  );
}
