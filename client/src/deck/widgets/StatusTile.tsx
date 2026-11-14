import type { StatusItemTile } from "@decka/shared";
import { useAppState } from "../../store/store";
import { keyColors } from "../look";
import { useKeySize } from "../useKeySize";
import { STATE_COLOR } from "./statusOrder";

/** A 1x1 status drawn like a key: tinted square with a state dot, the label below it. */
export default function StatusTile({ item }: { item: StatusItemTile }) {
  const status = useAppState((s) => s.state?.statuses.find((x) => x.id === item.statusId));
  const [cell, side] = useKeySize();
  // A missing status is shown as unknown, labelled with its id.
  const state = status?.state ?? "unknown";
  const color = STATE_COLOR[state];
  const dot = Math.round(0.2 * side);
  return (
    <div ref={cell} className="status-cell">
      <div
        className="key"
        style={{ width: side, height: side, borderRadius: 0.22 * side, background: keyColors(color).background, borderColor: keyColors(color).border }}
      >
        <span className={state === "running" ? "status-key-dot pulse" : "status-key-dot"} style={{ width: dot, height: dot, background: color }} />
      </div>
      <div className="key-label" style={{ marginTop: 0.06 * side, fontSize: Math.max(11, 0.16 * side) }}>
        {status?.label ?? item.statusId}
      </div>
      {status?.text && (
        <div className="key-label key-sub" style={{ fontSize: Math.max(10, 0.13 * side) }}>
          {status.text}
        </div>
      )}
    </div>
  );
}
