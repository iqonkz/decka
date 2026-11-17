import type { LayoutItem } from "@decka/shared";
import Button from "./Button";
import Ai from "./widgets/Ai";
import Twitch from "./widgets/Twitch";
import Youtube from "./widgets/Youtube";
import Clock from "./widgets/Clock";
import Media from "./widgets/Media";
import Notifications from "./widgets/Notifications";
import Obs from "./widgets/Obs";
import Timer from "./widgets/Timer";
import Volume from "./widgets/Volume";
import Weather from "./widgets/Weather";
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
    case "volume":
      content = <Volume item={item} />;
      break;
    case "timer":
      content = <Timer item={item} />;
      break;
    case "weather":
      content = <Weather item={item} />;
      break;
    case "obs":
      content = <Obs item={item} />;
      break;
    case "ai":
      content = <Ai item={item} />;
      break;
    case "twitch":
      content = <Twitch item={item} />;
      break;
    case "youtube":
      content = <Youtube item={item} />;
      break;
    default:
      content = <div className="tile-body">?</div>;
  }
  // Keys draw their own face; every widget is a panel whose face the theme defines.
  const panel = item.kind !== "button";
  return (
    <div className={panel ? "tile panel" : "tile"} data-tile-id={item.id} style={{ gridColumn: `${item.x + 1} / span ${item.w}`, gridRow: `${item.y + 1} / span ${item.h}` }}>
      {content}
    </div>
  );
}
