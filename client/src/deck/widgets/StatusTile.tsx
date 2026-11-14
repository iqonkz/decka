import type { StatusItemTile } from "@decka/shared";
import { useAppState } from "../../store/store";
import { useDeckMetrics } from "../metricsContext";
import { STATE_COLOR } from "./statusOrder";

/** A 1x1 status: a panel with a state dot, the label and the text. */
export default function StatusTile({ item }: { item: StatusItemTile }) {
  const status = useAppState((s) => s.state?.statuses.find((x) => x.id === item.statusId));
  const { side } = useDeckMetrics();
  // A missing status is shown as unknown, labelled with its id.
  const state = status?.state ?? "unknown";
  const color = STATE_COLOR[state];
  const dot = Math.round(0.2 * side);
  return (
    <div className="status-cell">
      <span className={state === "running" ? "status-key-dot pulse" : "status-key-dot"} style={{ width: dot, height: dot, background: color }} />
      <div className="key-caption" style={{ marginTop: 0.06 * side, fontSize: Math.max(11, 0.14 * side) }}>
        {status?.label ?? item.statusId}
      </div>
      {status?.text && (
        <div className="key-caption key-sub" style={{ fontSize: Math.max(10, 0.12 * side) }}>
          {status.text}
        </div>
      )}
    </div>
  );
}
