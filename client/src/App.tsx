import { useEffect } from "react";
import { t } from "@decka/shared";
import { useApplyLanguage } from "@decka/ui";
import { retryConnection, unpair } from "./app/controller";
import ConnectingScreen from "./screens/ConnectingScreen";
import LinkCard, { useLinkView } from "./screens/LinkCard";
import DeckScreen from "./screens/DeckScreen";
import PairScreen from "./screens/PairScreen";
import { applyOrientation } from "./platform/orientation";
import { useAppState } from "./store/store";
import ServiceCard from "./ui/ServiceCard";

export default function App() {
  const conn = useAppState((s) => s.conn);
  const hasState = useAppState((s) => s.state !== null);
  // `display.language` comes with the layout; before the pairing it is `auto` (the system language).
  const setting = useAppState((s) => s.state?.layout.display.language);
  useApplyLanguage(setting, navigator.language);
  const link = useLinkView();
  // `display.orientation` of the layout locks the screen and is remembered for the next start (DATA_MODEL.md 3.7).
  const orientation = useAppState((s) => s.state?.layout.display.orientation);
  useEffect(() => {
    if (hasState) applyOrientation(orientation);
  }, [hasState, orientation]);

  if (conn === "unpaired" || conn === "repair") return <PairScreen />;
  if (conn === "update_required") {
    return (
      <ServiceCard icon="arrow-circle-up" title={t("phone.update_title")} text={t("phone.update_required")}>
        <div className="service-actions">
          <button className="pill-btn primary" type="button" onClick={() => location.reload()}>
            {t("menu.reload")}
          </button>
          <button className="pill-btn secondary" type="button" onClick={unpair}>
            {t("phone.unpair")}
          </button>
        </div>
      </ServiceCard>
    );
  }
  if (conn === "not_your_pc") {
    // The PC could not prove it holds our key (PROTOCOL.md 10): no automatic retries until the owner decides.
    return (
      <ServiceCard icon="shield-warning" title={t("phone.not_your_pc_title")} text={t("phone.not_your_pc")}>
        <div className="service-actions">
          <button className="pill-btn primary" type="button" onClick={retryConnection}>
            {t("phone.retry")}
          </button>
          <button className="pill-btn secondary" type="button" onClick={unpair}>
            {t("phone.unpair")}
          </button>
        </div>
      </ServiceCard>
    );
  }
  return (
    <>
      {hasState ? <DeckScreen /> : <ConnectingScreen />}
      {link.card && <LinkCard reason={link.reason} />}
    </>
  );
}
