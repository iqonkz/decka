// The namespace, not `icons`: some names (`Music`) are only exported by name in lucide-react 1.x.
import * as lucideIcons from "lucide-react";
import PhIcon from "./PhIcon";

type Glyph = React.ComponentType<{ size?: number }>;

/** `volume-2` -> `Volume2`, the key of lucide-react's `icons`. */
const pascal = (name: string): string => name.split("-").map((p) => p.charAt(0).toUpperCase() + p.slice(1)).join("");

/** The icon of a library entry (DATA_MODEL.md 9): `lucide:<name>` or `ph:<name>`, in the colour of the text. */
export default function EntryIcon({ icon, size = 18 }: { icon: string; size?: number }) {
  if (icon.startsWith("ph:")) return <PhIcon name={icon.slice(3)} size={size} />;
  const Glyph = (lucideIcons as unknown as Record<string, Glyph>)[pascal(icon.replace(/^lucide:/, ""))];
  return Glyph ? <Glyph size={size} /> : <span style={{ display: "inline-block", width: size, height: size }} />;
}
