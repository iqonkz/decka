import type { StatusItemTile } from "@decka/shared";
import { useAppState } from "../../store/store";
import { STATE_COLOR } from "./statusOrder";

export default function StatusTile({ item }: { item: StatusItemTile }) {
  const status = useAppState((s) => s.state?.statuses.find((x) => x.id === item.statusId));
  // A missing status is shown as unknown, labelled with its id.
  const state = status?.state ?? "unknown";
  const color = STATE_COLOR[state];
  return (
    <div
      className="status-tile"
      style={{ background: `${color}40`, borderColor: color }} // #rrggbb + 0x40 alpha = 25%
    >
      <div className={state === "running" ? "status-tile-label pulse" : "status-tile-label"}>{status?.label ?? item.statusId}</div>
      {status?.text && <div className="status-tile-text">{status.text}</div>}
    </div>
  );
}
