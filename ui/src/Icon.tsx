import { useDeckEnv } from "./deckEnv";
import { FALLBACK_ICON, ICONS } from "./icons";

const IMG_RE = /^img:([0-9a-f]{16})$/;

/**
 * `lucide:<name>`, `emoji:<symbol>` or `img:<hash16>` (a picture from the `icons` section); `size` is in px.
 * A bitmap (PNG) picture of a program may be larger than a line icon: `bitmapSize` (DESIGN.md 2), default `size`.
 * A vector picture (SVG from Phosphor) is drawn as a mask in the colour of the text, so it follows theme and key colour.
 */
export default function Icon({ icon, size, bitmapSize }: { icon: string | undefined; size: number; bitmapSize?: number }) {
  const { icons } = useDeckEnv();
  if (!icon) return null;
  const img = IMG_RE.exec(icon);
  if (img) {
    const url = icons[img[1] as string];
    if (url && url.startsWith("data:image/svg+xml")) {
      const mask = `url("${url}")`;
      return (
        <span
          aria-hidden="true"
          style={{
            display: "block",
            width: size,
            height: size,
            backgroundColor: "currentColor",
            WebkitMaskImage: mask,
            maskImage: mask,
            WebkitMaskSize: "contain",
            maskSize: "contain",
            WebkitMaskRepeat: "no-repeat",
            maskRepeat: "no-repeat",
            WebkitMaskPosition: "center",
            maskPosition: "center",
            pointerEvents: "none",
          }}
        />
      );
    }
    if (url) {
      const px = bitmapSize ?? size;
      return <img src={url} width={px} height={px} draggable={false} alt="" style={{ objectFit: "contain", borderRadius: Math.round(0.16 * px), pointerEvents: "none" }} />;
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
