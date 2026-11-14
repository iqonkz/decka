import type { ButtonItem } from "@decka/shared";
import { setPage } from "../app/controller";
import { getState, useAppState } from "../store/store";
import Key from "./Key";
import { resolveLook } from "./look";

export default function Button({ item }: { item: ButtonItem }) {
  const vars = useAppState((s) => s.state?.vars);
  const look = resolveLook(item, vars);

  function navigate() {
    if (item.navigate === undefined) return;
    const index = getState().state?.layout.pages.findIndex((p) => p.id === item.navigate) ?? -1;
    if (index >= 0) setPage(index);
  }

  return (
    <Key
      {...(item.actionId !== undefined ? { actionId: item.actionId } : {})}
      {...(item.confirm !== undefined ? { confirm: item.confirm } : {})}
      {...(look.icon !== undefined ? { icon: look.icon } : {})}
      {...(look.label !== undefined ? { label: look.label } : {})}
      {...(look.color !== undefined ? { color: look.color } : {})}
      onTap={navigate}
    />
  );
}
