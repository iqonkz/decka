import type { LayoutItem } from "@decka/shared";
import Button from "./Button";
import Clock from "./widgets/Clock";
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
    case "status":
    case "notifications":
      content = <div className="tile-body">{kind}</div>; // widgets arrive in M5
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
