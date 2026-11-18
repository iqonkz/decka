import { useEffect, useState } from "react";
import { t } from "@decka/shared";
import { useAppState } from "../store/store";

/** How long the plain "Connecting" stays before the checklist shows. */
export const HINT_AFTER_MS = 8000;

/** Before the first state arrives: `Connecting…`, and after a few seconds what usually blocks the connection. */
export default function ConnectingScreen() {
  const conn = useAppState((s) => s.conn);
  const error = useAppState((s) => s.lastError);
  const [slow, setSlow] = useState(false);
  useEffect(() => {
    const timer = window.setTimeout(() => setSlow(true), HINT_AFTER_MS);
    return () => window.clearTimeout(timer);
  }, []);
  return (
    <div className="center connecting">
      <div>{conn === "offline" ? t("phone.no_pc") : t("phone.connecting")}</div>
      {slow && (
        <ul className="connecting-hints">
          <li>{t("phone.hint_wifi")}</li>
          <li>{t("phone.hint_agent")}</li>
          <li>{t("phone.hint_firewall")}</li>
          {error && <li className="connecting-error">{error}</li>}
        </ul>
      )}
    </div>
  );
}
