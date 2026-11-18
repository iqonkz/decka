import { useEffect } from "react";
import { t } from "@decka/shared";
import { useApplyLanguage } from "@decka/ui";
import ConnectingScreen from "./screens/ConnectingScreen";
import DeckScreen from "./screens/DeckScreen";
import PairScreen from "./screens/PairScreen";
import { applyOrientation } from "./platform/orientation";
import { useAppState } from "./store/store";

export default function App() {
  const conn = useAppState((s) => s.conn);
  const hasState = useAppState((s) => s.state !== null);
  // `display.language` comes with the layout; before the pairing it is `auto` (the system language).
  const setting = useAppState((s) => s.state?.layout.display.language);
  useApplyLanguage(setting, navigator.language);
  // `display.orientation` of the layout locks the screen and is remembered for the next start (DATA_MODEL.md 3.7).
  const orientation = useAppState((s) => s.state?.layout.display.orientation);
  useEffect(() => {
    if (hasState) applyOrientation(orientation);
  }, [hasState, orientation]);

  if (conn === "unpaired") return <PairScreen />;
  if (conn === "update_required") return <div className="center">{t("phone.update_required")}</div>;
  if (!hasState) return <ConnectingScreen />;
  return <DeckScreen />;
}
