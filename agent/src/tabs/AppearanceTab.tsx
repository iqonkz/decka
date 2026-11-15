import { useEffect, useState } from "react";
import type { DisplaySettings, LayoutFile, ValidationIssue } from "@decka/shared";
import { call } from "../ipc";
import Banner from "../ui/Banner";
import Button from "../ui/Button";
import Field from "../ui/Field";
import ThemeCards from "./ThemeCards";

export const ACCENTS = ["#7c5cff", "#3b82f6", "#06b6d4", "#16a34a", "#eab308", "#f97316", "#ef4444", "#ec4899"];
const WALLPAPERS = ["aurora", "sunset", "ocean", "graphite", "album"] as const;

/** Look of the deck: theme, accent, wallpaper, context pages and the display (dimming, screen saver). */
export default function AppearanceTab() {
  const [display, setDisplay] = useState<DisplaySettings | null>(null);
  const [saved, setSaved] = useState("");
  const [issues, setIssues] = useState<ValidationIssue[]>([]);
  const [message, setMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    call("get_layout").then(
      (l) => {
        setDisplay(l.display);
        setSaved(JSON.stringify(l.display));
      },
      (e) => setError(String(e)),
    );
  }, []);

  if (error) return <Banner kind="error">{error}</Banner>;
  if (!display) return <p>Loading…</p>;

  const set = (patch: Partial<DisplaySettings>) => {
    setDisplay({ ...display, ...patch });
    setMessage(null);
    setIssues([]);
  };
  const dirty = JSON.stringify(display) !== saved;
  const accent = display.accent ?? "#7c5cff";

  async function save() {
    // The layout may have been edited in the Layout tab meanwhile: only the display part is replaced.
    const fresh: LayoutFile = await call("get_layout");
    const result = await call("save_layout", { layout: { ...fresh, display: display! } });
    if (result.ok) {
      setSaved(JSON.stringify(display));
      setMessage("Saved. The phone updates within a second.");
      setIssues([]);
    } else {
      setIssues(result.issues);
      setMessage(null);
    }
  }

  return (
    <div className="stack">
      <h2>Appearance</h2>

      <section className="card">
        <h3>Look</h3>
        <ThemeCards value={display.theme ?? "amoled"} display={display} onPick={(theme) => set({ theme })} />
      </section>

      <section className="card">
        <h3>Accent colour</h3>
        <p className="muted">Colours the Material look and this window.</p>
        <div className="row wrap">
          {ACCENTS.map((c) => (
            <button key={c} type="button" className={accent.toLowerCase() === c ? "swatch selected" : "swatch"} style={{ background: c }} aria-label={`Accent ${c}`} onClick={() => set({ accent: c })} />
          ))}
          <label className="check">
            <input type="color" value={accent} onChange={(e) => set({ accent: e.target.value })} aria-label="Custom accent" /> Custom
          </label>
        </div>
      </section>

      {display.theme === "glass" && (
        <section className="card">
          <h3>Wallpaper</h3>
          <p className="muted">Behind the glass. “album” shows the cover of the current track.</p>
          <div className="row wrap">
            {WALLPAPERS.map((w) => (
              <label key={w} className="check">
                <input type="radio" name="wallpaper" checked={(display.wallpaper ?? "aurora") === w} onChange={() => set({ wallpaper: w })} /> {w}
              </label>
            ))}
          </div>
        </section>
      )}

      <section className="card">
        <h3>Pages</h3>
        <label className="check">
          <input type="checkbox" checked={display.contextPages !== false} onChange={(e) => set({ contextPages: e.target.checked })} /> Switch to app pages automatically
        </label>
        <p className="muted">The phone opens the page of the program that is in front on your PC and returns when you leave it.</p>
      </section>

      <section className="card">
        <h3>Screen</h3>
        <div className="grid-4">
          <Field label="Dim after, s" hint="0 = never">
            <input type="number" value={display.dimAfterSec} onChange={(e) => set({ dimAfterSec: Number(e.target.value) })} />
          </Field>
          <Field label="Dim level" hint="0.05 to 1">
            <input type="number" step="0.05" value={display.dimLevel} onChange={(e) => set({ dimLevel: Number(e.target.value) })} />
          </Field>
          <Field label="Screen saver after, s" hint="0 = never">
            <input type="number" value={display.sleepAfterSec} onChange={(e) => set({ sleepAfterSec: Number(e.target.value) })} />
          </Field>
          <label className="check">
            <input type="checkbox" checked={display.pixelShift} onChange={(e) => set({ pixelShift: e.target.checked })} /> Pixel shift
          </label>
        </div>
        <div className="row">
          <label className="check">
            <input type="checkbox" checked={display.night.enabled} onChange={(e) => set({ night: { ...display.night, enabled: e.target.checked } })} /> Night mode
          </label>
          <input aria-label="Night from" value={display.night.from} placeholder="23:00" onChange={(e) => set({ night: { ...display.night, from: e.target.value } })} />
          <span>to</span>
          <input aria-label="Night to" value={display.night.to} placeholder="07:00" onChange={(e) => set({ night: { ...display.night, to: e.target.value } })} />
        </div>
      </section>

      {issues.length > 0 && (
        <Banner kind="error">
          <ul>
            {issues.map((i, n) => (
              <li key={n}>{i.message}</li>
            ))}
          </ul>
        </Banner>
      )}
      {message && <Banner kind="success">{message}</Banner>}
      <div className="row">
        <Button variant="primary" disabled={!dirty} onClick={() => void save()}>
          Save
        </Button>
        {dirty && <span className="unsaved">Unsaved changes</span>}
      </div>
    </div>
  );
}
