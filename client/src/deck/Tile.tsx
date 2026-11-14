import type { LayoutItem } from "@decka/shared";
import Button from "./Button";
import { PLAIN_FACE } from "./look";
import { useDeckMetrics } from "./metricsContext";
import Clock from "./widgets/Clock";
import Media from "./widgets/Media";
import Notifications from "./widgets/Notifications";
import Statuses from "./widgets/Statuses";
import StatusTile from "./widgets/StatusTile";
import Sys from "./widgets/Sys";

/** Common tile container; the content depends on `kind`. */
export default function Tile({ item }: { item: LayoutItem }) {
  // `kind` comes from the PC and may be newer than this client: unknown kinds render as "?".
  const kind: string = item.kind;
  let content;
  switch (item.kind) {
    case "button":
      content = <Button item={item} />;
      break;
    case "clock":
      content = <Clock item={item} />;
      break;
    case "sys":
      content = <Sys item={item} />;
      break;
    case "statuses":
      content = <Statuses item={item} />;
      break;
    case "status":
      content = <StatusTile item={item} />;
      break;
    case "notifications":
      content = <Notifications item={item} />;
      break;
    case "media":
      content = <Media item={item} />;
      break;
    default:
      content = <div className="tile-body">?</div>;
  }
  const { side } = useDeckMetrics();
  // Keys draw their own face; every widget is a panel with the same plain face (padding 8% of a key).
  const panel = item.kind !== "button";
  return (
    <div
      className={panel ? "tile panel" : "tile"}
      style={{
        gridColumn: `${item.x + 1} / span ${item.w}`,
        gridRow: `${item.y + 1} / span ${item.h}`,
        ...(panel ? { borderRadius: 0.16 * side, padding: 0.08 * side, background: PLAIN_FACE.background, borderColor: PLAIN_FACE.border } : {}),
      }}
    >
      {content}
    </div>
  );
}
