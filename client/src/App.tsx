import DeckScreen from "./screens/DeckScreen";
import PairScreen from "./screens/PairScreen";
import { useAppState } from "./store/store";

export default function App() {
  const conn = useAppState((s) => s.conn);
  const hasState = useAppState((s) => s.state !== null);

  if (conn === "unpaired") return <PairScreen />;
  if (conn === "update_required") return <div className="center">Update the Decka app to continue.</div>;
  if (!hasState) return <div className="center">Connecting…</div>;
  return <DeckScreen />;
}
