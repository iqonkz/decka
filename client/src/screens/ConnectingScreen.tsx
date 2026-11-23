import { t } from "@decka/shared";
import { useAppState } from "../store/store";

/** Before the first state arrives: `Connecting…`. After 8 s the card with the way out comes over it (LinkCard, DESIGN.md 4.13). */
export default function ConnectingScreen() {
  const conn = useAppState((s) => s.conn);
  const error = useAppState((s) => s.lastError);
  return (
    <div className="center connecting">
      <div>{conn === "offline" ? t("phone.no_pc") : t("phone.connecting")}</div>
      {error && <p className="connecting-error">{error}</p>}
    </div>
  );
}
