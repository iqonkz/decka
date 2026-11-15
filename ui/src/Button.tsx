import type { ButtonItem } from "@decka/shared";
import { useDeckEnv } from "./deckEnv";
import Key from "./Key";
import { resolveLook } from "./look";

export default function Button({ item }: { item: ButtonItem }) {
  const { vars, onNavigate } = useDeckEnv();
  const look = resolveLook(item, vars);

  return (
    <Key
      {...(item.actionId !== undefined ? { actionId: item.actionId } : {})}
      {...(item.confirm !== undefined ? { confirm: item.confirm } : {})}
      {...(look.icon !== undefined ? { icon: look.icon } : {})}
      {...(look.label !== undefined ? { label: look.label } : {})}
      {...(look.color !== undefined ? { color: look.color } : {})}
      {...(item.navigate !== undefined ? { chevron: true } : {})}
      onTap={() => item.navigate !== undefined && onNavigate(item.navigate)}
    />
  );
}
