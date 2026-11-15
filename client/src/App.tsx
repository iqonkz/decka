import { t } from "@decka/shared";
import { useApplyLanguage } from "@decka/ui";
import DeckScreen from "./screens/DeckScreen";
import PairScreen from "./screens/PairScreen";
import { useAppState } from "./store/store";

export default function App() {
  const conn = useAppState((s) => s.conn);
  const hasState = useAppState((s) => s.state !== null);
  // `display.language` comes with the layout; before the pairing it is `auto` (the system language).
  const setting = useAppState((s) => s.state?.layout.display.language);
  useApplyLanguage(setting, navigator.language);

  if (conn === "unpaired") return <PairScreen />;
  if (conn === "update_required") return <div className="center">{t("phone.update_required")}</div>;
  if (!hasState) return <div className="center">{t("phone.connecting")}</div>;
  return <DeckScreen />;
}
