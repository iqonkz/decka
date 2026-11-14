import { FALLBACK_ICON, ICONS } from "./icons";

/** `lucide:<name>` or `emoji:<symbol>`; `size` is in px. */
export default function Icon({ icon, size }: { icon: string | undefined; size: number }) {
  if (!icon) return null;
  if (icon.startsWith("lucide:")) {
    const Component = ICONS[icon.slice("lucide:".length)] ?? FALLBACK_ICON;
    return <Component size={size} strokeWidth={1.75} />;
  }
  if (icon.startsWith("emoji:")) {
    return <span style={{ fontSize: size, lineHeight: 1 }}>{icon.slice("emoji:".length)}</span>;
  }
  return null;
}
