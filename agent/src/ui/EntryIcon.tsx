import { ICONS } from "@decka/ui";
import PhIcon from "./PhIcon";

/**
 * The icon of a library entry (DATA_MODEL.md 9): `lucide:<name>` or `ph:<name>`, in the colour of the text. Only the lucide
 * icons the phone draws are known: the whole library would double the window (T31.5).
 */
export default function EntryIcon({ icon, size = 18 }: { icon: string; size?: number }) {
  if (icon.startsWith("ph:")) return <PhIcon name={icon.slice(3)} size={size} />;
  const Glyph = ICONS[icon.replace(/^lucide:/, "")];
  return Glyph ? <Glyph size={size} /> : <span style={{ display: "inline-block", width: size, height: size }} />;
}
