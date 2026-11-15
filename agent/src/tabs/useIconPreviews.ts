import { useEffect, useMemo, useState } from "react";
import type { Page } from "@decka/shared";
import { call } from "../ipc";

/** Pictures of the `img:` icons used on a page, for the preview (`hash -> data URL`). */
export function useIconPreviews(page: Page): Record<string, string> {
  const [icons, setIcons] = useState<Record<string, string>>({});
  const wanted = useMemo(() => {
    const out = new Set<string>();
    for (const item of page.items) {
      if (item.kind !== "button") continue;
      for (const icon of [item.icon, ...Object.values(item.bind?.states ?? {}).map((s) => s.icon)]) if (icon?.startsWith("img:")) out.add(icon);
    }
    return [...out];
  }, [page]);
  useEffect(() => {
    let live = true;
    for (const icon of wanted) {
      if (icons[icon.slice(4)]) continue;
      call("get_icon_preview", { icon }).then(
        (url) => live && url && setIcons((prev) => ({ ...prev, [icon.slice(4)]: url })),
        () => {},
      );
    }
    return () => {
      live = false;
    };
  }, [wanted]); // eslint-disable-line react-hooks/exhaustive-deps
  return icons;
}