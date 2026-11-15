import { useDeckEnv } from "./deckEnv";
import { FALLBACK_ICON, ICONS } from "./icons";

const IMG_RE = /^img:([0-9a-f]{16})$/;

/** `lucide:<name>`, `emoji:<symbol>` or `img:<hash16>` (a picture from the `icons` section); `size` is in px. */
export default function Icon({ icon, size }: { icon: string | undefined; size: number }) {
  const { icons } = useDeckEnv();
  if (!icon) return null;
  const img = IMG_RE.exec(icon);
  if (img) {
    const url = icons[img[1] as string];
    if (url) {
      return <img src={url} width={size} height={size} draggable={false} alt="" style={{ objectFit: "contain", borderRadius: Math.round(0.16 * size), pointerEvents: "none" }} />;
    }
    return <FALLBACK_ICON size={size} strokeWidth={1.75} />; // not (yet) in the section: a square
  }
  if (icon.startsWith("lucide:")) {
    const Component = ICONS[icon.slice("lucide:".length)] ?? FALLBACK_ICON;
    return <Component size={size} strokeWidth={1.75} />;
  }
  if (icon.startsWith("emoji:")) {
    return <span style={{ fontSize: size, lineHeight: 1 }}>{icon.slice("emoji:".length)}</span>;
  }
  return null;
}
