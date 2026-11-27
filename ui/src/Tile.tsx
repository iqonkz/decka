import { t, type LayoutItem } from "@decka/shared";
import { useDeckEnv } from "./deckEnv";
import Icon from "./Icon";
import Button from "./Button";
import Ai from "./widgets/Ai";
import Battery from "./widgets/Battery";
import Mail from "./widgets/Mail";
import ListWidget from "./widgets/ListWidget";
import Reminders from "./widgets/Reminders";
import VoiceKey from "./widgets/VoiceKey";
import Twitch from "./widgets/Twitch";
import Youtube from "./widgets/Youtube";
import Clock from "./widgets/Clock";
import Context from "./widgets/Context";
import Media from "./widgets/Media";
import Notifications from "./widgets/Notifications";
import Obs from "./widgets/Obs";
import Timer from "./widgets/Timer";
import Volume from "./widgets/Volume";
import Weather from "./widgets/Weather";
import Statuses from "./widgets/Statuses";
import StatusTile from "./widgets/StatusTile";
import Sys from "./widgets/Sys";
import Calendar from "./widgets/Calendar";
import { hasWidgetHeader } from "./widgets/density";

const WIDGET_ICONS: Record<string, string> = {
  clock: "lucide:clock",
  sys: "lucide:cpu",
  statuses: "lucide:activity",
  status: "lucide:circle",
  notifications: "lucide:bell",
  media: "lucide:music",
  volume: "lucide:volume-2",
  timer: "lucide:timer",
  weather: "lucide:sun",
  obs: "lucide:video",
  ai: "lucide:terminal",
  twitch: "lucide:video",
  youtube: "lucide:play",
  battery: "lucide:bluetooth",
  mail: "lucide:mail",
  reminders: "lucide:calendar",
  calendar: "lucide:calendar",
  list: "lucide:check",
};

/** Common tile container; the content depends on `kind`. */
export default function Tile({ item }: { item: LayoutItem }) {
  const { lists } = useDeckEnv();
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
    case "battery":
      content = <Battery item={item} />;
      break;
    case "mail":
      content = <Mail item={item} />;
      break;
    case "context":
      content = <Context item={item} />;
      break;
    case "voice":
      content = <VoiceKey item={item} />;
      break;
    case "reminders":
      content = <Reminders item={item} />;
      break;
    case "list":
      content = <ListWidget item={item} />;
      break;
    default:
      content = kind === "calendar" ? <Calendar item={item} /> : <div className="tile-body">?</div>;
  }
  // Keys draw their own face; every widget is a panel whose face the theme defines.
  // The context widget is a bare grid of keys (its empty hint draws its own frame).
  const panel = kind !== "button" && kind !== "context" && kind !== "voice";
  const showHeader = panel && hasWidgetHeader(item);
  let headerTitle = "";
  if (showHeader) {
    if (item.kind === "statuses" && item.title) {
      headerTitle = item.title;
    } else if (item.kind === "list") {
      const all = lists?.lists ?? [];
      const found = item.listId === undefined ? all[0] : all.find((l) => l.id === item.listId);
      headerTitle = found?.name ?? t("widget_name.list");
    } else if (kind in WIDGET_ICONS) {
      headerTitle = t(`widget_name.${kind}` as Parameters<typeof t>[0]);
    }
  }
  const headerIcon = WIDGET_ICONS[kind] ?? "lucide:square";

  return (
    <div className={panel ? "tile panel" : "tile"} data-tile="" data-tile-id={item.id} style={{ gridColumn: `${item.x + 1} / span ${item.w}`, gridRow: `${item.y + 1} / span ${item.h}` }}>
      {showHeader && (
        <div className="tile-head">
          <Icon icon={headerIcon} size={14} />
          <span className="tile-head-title">{headerTitle}</span>
        </div>
      )}
      {panel ? <div className="tile-content">{content}</div> : content}
    </div>
  );
}
