import type { LayoutItem } from "@decka/shared";
import Button from "./Button";
import Clock from "./widgets/Clock";
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
    default:
      content = <div className="tile-body">?</div>;
  }
  return (
    <div
      className="tile"
      style={{ gridColumn: `${item.x + 1} / span ${item.w}`, gridRow: `${item.y + 1} / span ${item.h}` }}
    >
      {content}
    </div>
  );
}
