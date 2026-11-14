import type { LayoutItem } from "@decka/shared";
import Button from "./Button";

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
    case "sys":
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
