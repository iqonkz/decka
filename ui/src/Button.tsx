import type { ButtonItem } from "@decka/shared";
import { useDeckEnv } from "./deckEnv";
import Key from "./Key";
import { labelShown, resolveFace } from "./faceLayout";
import { resolveLook } from "./look";

export default function Button({ item }: { item: ButtonItem }) {
  const { vars, onNavigate, onOpenPopup, keyFace, keyLabels } = useDeckEnv();
  const look = resolveLook(item, vars);
  const open = (box: DOMRect) => item.popup !== undefined && onOpenPopup?.(item.popup, { x: box.x, y: box.y, width: box.width, height: box.height });

  return (
    <Key
      {...(item.actionId !== undefined ? { actionId: item.actionId } : {})}
      {...(item.confirm !== undefined ? { confirm: item.confirm } : {})}
      {...(look.icon !== undefined ? { icon: look.icon } : {})}
      {...(labelShown(look.label, item.showLabel, keyLabels) ? { label: look.label as string } : {})}
      face={resolveFace(item.face, keyFace)}
      {...(look.color !== undefined ? { color: look.color } : {})}
      {...(item.navigate !== undefined ? { chevron: true } : {})}
      {...(item.popup !== undefined ? { dots: true } : {})}
      onTap={(box) => (item.navigate !== undefined ? onNavigate(item.navigate) : open(box))}
      onPressed={open}
    />
  );
}
