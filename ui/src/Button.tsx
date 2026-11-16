import { BACK_PAGE, isReservedVar, t, type ButtonItem } from "@decka/shared";
import { useDeckEnv } from "./deckEnv";
import Key from "./Key";
import { labelShown, resolveFace } from "./faceLayout";
import { resolveLook } from "./look";

/** Variables whose `true` means something is on the air: the key gets the blinking red dot (DESIGN.md 2.4). */
const LIVE_VARS = new Set(["obs.recording", "obs.streaming"]);

/**
 * A key is dimmed (0.45) only when it follows a reserved collector variable that does not exist (no OBS, no microphone);
 * a user variable that is not set yet is the ordinary state, and the settings preview never dims (DESIGN.md 2.4).
 */
export function keyHasNoData(item: ButtonItem, vars: Record<string, unknown>, preview: boolean): boolean {
  return !preview && item.bind !== undefined && isReservedVar(item.bind.var) && vars[item.bind.var] === undefined;
}

export default function Button({ item }: { item: ButtonItem }) {
  const { vars, onNavigate, onOpenPopup, keyFace, keyLabels, preview } = useDeckEnv();
  const back = item.navigate === BACK_PAGE;
  const look = resolveLook(item, vars);
  // The back key looks like one without being told: an arrow and `Back` (DATA_MODEL.md 3.8).
  const icon = look.icon ?? (back ? "lucide:arrow-left" : undefined);
  const label = look.label ?? (back ? t("nav.back") : undefined);
  const noData = keyHasNoData(item, vars, preview === true);
  const live = item.bind !== undefined && LIVE_VARS.has(item.bind.var) && vars[item.bind.var] === true;
  const open = (box: DOMRect) => item.popup !== undefined && onOpenPopup?.(item.popup, { x: box.x, y: box.y, width: box.width, height: box.height });

  return (
    <Key
      {...(item.actionId !== undefined ? { actionId: item.actionId } : {})}
      {...(item.confirm !== undefined ? { confirm: item.confirm } : {})}
      {...(icon !== undefined ? { icon } : {})}
      {...(labelShown(label, item.showLabel, keyLabels) ? { label: label as string } : {})}
      face={resolveFace(item.face, keyFace)}
      {...(look.color !== undefined ? { color: look.color } : {})}
      {...(item.navigate !== undefined && !back ? { chevron: true } : {})}
      {...(item.popup !== undefined ? { dots: true } : {})}
      {...(noData ? { noData: true } : {})}
      {...(live ? { liveDot: true } : {})}
      onTap={(box) => (item.navigate !== undefined ? onNavigate(item.navigate) : open(box))}
      onPressed={open}
    />
  );
}
