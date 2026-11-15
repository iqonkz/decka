import { useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { icons as lucideIcons } from "lucide-react";
import { call } from "../ipc";
import Button from "../ui/Button";
import { ICON_NAMES } from "./iconNames";

/** `volume-2` -> `Volume2`, the key of lucide-react's `icons`. */
function pascal(name: string): string {
  return name.split("-").map((p) => p.charAt(0).toUpperCase() + p.slice(1)).join("");
}

/** What the current icon looks like: the picture, the Lucide glyph, the emoji, or an empty box. */
export function IconPreview({ icon }: { icon: string | undefined }) {
  const [url, setUrl] = useState<string | null>(null);
  useEffect(() => {
    setUrl(null);
    if (!icon?.startsWith("img:")) return;
    let live = true;
    call("get_icon_preview", { icon }).then((u) => live && setUrl(u), () => live && setUrl(null));
    return () => {
      live = false;
    };
  }, [icon]);

  let content: React.ReactNode = null;
  if (icon?.startsWith("img:")) content = url ? <img src={url} width={32} height={32} alt="" /> : <span className="muted">?</span>;
  else if (icon?.startsWith("lucide:")) {
    const Glyph = (lucideIcons as Record<string, React.ComponentType<{ size?: number }>>)[pascal(icon.slice(7))];
    content = Glyph ? <Glyph size={24} /> : <span className="muted">{icon.slice(7)}</span>;
  } else if (icon?.startsWith("emoji:")) content = <span style={{ fontSize: 22 }}>{icon.slice(6)}</span>;
  return <span className="icon-preview">{content}</span>;
}

interface Props {
  value: string | undefined;
  onChange: (icon: string | undefined) => void;
  /** Program of the button's `launch` action; enables "From program". `undefined` hides the button (state looks). */
  program?: string | null;
}

/** Icon of a button: preview, Lucide list, the icon of the action's program, a picture file, or none. */
export default function IconField({ value, onChange, program }: Props) {
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  async function fromResult(promise: Promise<{ icon: string } | { error: string }>) {
    setBusy(true);
    setError(null);
    try {
      const result = await promise;
      if ("icon" in result) onChange(result.icon);
      else setError(result.error);
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }

  async function fromImage() {
    const picked = await open({ multiple: false, directory: false, filters: [{ name: "Images", extensions: ["png", "jpg", "jpeg", "ico", "bmp"] }] });
    if (typeof picked === "string") await fromResult(call("icon_from_image", { path: picked }));
  }

  return (
    <div className="icon-field">
      <div className="row">
        <IconPreview icon={value} />
        <input value={value ?? ""} aria-label="Icon" placeholder="lucide:play, emoji:🚀 or a picture" onChange={(e) => onChange(e.target.value || undefined)} />
      </div>
      <div className="row wrap">
        <select value="" aria-label="Lucide icon" onChange={(e) => e.target.value && onChange(`lucide:${e.target.value}`)}>
          <option value="">Lucide…</option>
          {ICON_NAMES.map((n) => (
            <option key={n} value={n}>
              {n}
            </option>
          ))}
        </select>
        {program !== undefined && (
          <Button disabled={busy || !program} onClick={() => void fromResult(call("icon_from_program", { path: program as string }))}>
            From program
          </Button>
        )}
        <Button disabled={busy} onClick={() => void fromImage()}>
          From image…
        </Button>
        <Button disabled={value === undefined} onClick={() => { setError(null); onChange(undefined); }}>
          Clear
        </Button>
      </div>
      {error && <p className="warn-text">{error}</p>}
    </div>
  );
}
