import PairScreen from "./screens/PairScreen";
import { useAppState } from "./store/store";

export default function App() {
  const conn = useAppState((s) => s.conn);
  const agentName = useAppState((s) => s.agentName);

  if (conn === "unpaired") return <PairScreen />;
  if (conn === "update_required") return <div className="center">Update the Decka app to continue.</div>;
  // Placeholder until the grid arrives in T4.4.
  return <div className="center">{agentName ? `Connected to ${agentName}` : "Connecting…"}</div>;
}
