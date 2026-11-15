import { useEffect, useState } from "react";
import type { DisplaySettings, LayoutFile, ValidationIssue } from "@decka/shared";
import { call } from "../ipc";
import Banner from "../ui/Banner";
import Button from "../ui/Button";
import Field from "../ui/Field";
import KeyPreview from "./KeyPreview";
import { applyScreenPreset, fracToPercent, minToSec, percentToFrac, PRESET_LABELS, screenPreset, secToMin, type ScreenPreset } from "../editor/screenPreset";
import ThemeCards from "./ThemeCards";

/** A sample key for the style cards. */
const SAMPLE = { id: "sample", kind: "button", x: 0, y: 0, w: 1, h: 1, label: "Music", icon: "lucide:music", actionId: "sample" } as const;

export const ACCENTS = ["#7c5cff", "#3b82f6", "#06b6d4", "#16a34a", "#eab308", "#f97316", "#ef4444", "#ec4899"];
const WALLPAPERS = ["aurora", "sunset", "ocean", "graphite", "album"] as const;

/** Look of the deck: theme, accent, wallpaper, context pages and the display (dimming, screen saver). */
export default function AppearanceTab() {
  const [display, setDisplay] = useState<DisplaySettings | null>(null);
  const [saved, setSaved] = useState("");
  const [issues, setIssues] = useState<ValidationIssue[]>([]);
  const [message, setMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [lowres, setLowres] = useState(0);
  const [custom, setCustom] = useState(false);

  useEffect(() => {
    call("count_lowres_icons").then(setLowres, () => {});
  }, []);

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
  const preset = screenPreset(display);
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
      <h2>Look</h2>

      <section className="card">
        <h3>Theme</h3>
        <ThemeCards value={display.theme ?? "amoled"} display={display} onPick={(theme) => set({ theme })} />
      </section>

      <section className="card">
        <h3>Key style</h3>
        <div className="style-cards">
          {(["standard", "full"] as const).map((face) => (
            <button key={face} type="button" className={(display.keyFace ?? "standard") === face ? "style-card selected" : "style-card"} onClick={() => set({ keyFace: face })}>
              <KeyPreview item={SAMPLE} display={{ ...display, keyFace: face }} />
              <strong>{face === "standard" ? "Standard" : "Big icon"}</strong>
              <small className="muted">{face === "standard" ? "A small icon in a frame" : "The icon fills the key, like on a home screen"}</small>
            </button>
          ))}
        </div>
        <label className="check">
          <input type="checkbox" checked={display.keyLabels !== false} onChange={(e) => set({ keyLabels: e.target.checked })} /> Show names
        </label>
        {lowres > 0 && (
          <div className="row">
            <span className="muted">Some app icons are low-resolution —</span>
            <Button
              onClick={() =>
                void call("refresh_program_icons").then(async (r) => {
                  setLowres(await call("count_lowres_icons"));
                  setMessage(r.updated > 0 ? `Sharpened ${r.updated} icon${r.updated === 1 ? "" : "s"}.` : "Nothing to sharpen.");
                })
              }
            >
              Sharpen
            </Button>
          </div>
        )}
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
        <h3>Screen saving</h3>
        <div className="seg" role="group" aria-label="Screen saving">
          {(["off", "balanced", "maximum", "custom"] as ScreenPreset[]).map((p) => (
            <button key={p} type="button" className={(custom ? "custom" : preset) === p ? "on" : ""} onClick={() => (p === "custom" ? setCustom(true) : (setCustom(false), set(applyScreenPreset(display, p))))}>
              {PRESET_LABELS[p]}
            </button>
          ))}
        </div>
        <p className="muted">
          {preset === "balanced" && !custom && "Dims after 1 minute to 30%, shows a clock screen after 10 minutes."}
          {preset === "maximum" && !custom && "Dims after 30 seconds to 15%, shows a clock screen after 3 minutes."}
          {preset === "off" && !custom && "The screen stays as it is."}
          {(custom || preset === "custom") && "Your own times."}
        </p>
        {(custom || preset === "custom") && (
          <div className="grid-4">
            <Field label="Dim after, min" hint="0 = never">
              <input type="number" min={0} step={0.5} value={secToMin(display.dimAfterSec)} onChange={(e) => set({ dimAfterSec: minToSec(Number(e.target.value)) })} />
            </Field>
            <Field label="Brightness when dimmed, %" hint="5 to 100">
              <input type="number" min={5} max={100} value={fracToPercent(display.dimLevel)} onChange={(e) => set({ dimLevel: percentToFrac(Number(e.target.value)) })} />
            </Field>
            <Field label="Screen saver after, min" hint="0 = never">
              <input type="number" min={0} step={0.5} value={secToMin(display.sleepAfterSec)} onChange={(e) => set({ sleepAfterSec: minToSec(Number(e.target.value)) })} />
            </Field>
          </div>
        )}
        <label className="check">
          <input type="checkbox" checked={display.pixelShift} onChange={(e) => set({ pixelShift: e.target.checked })} /> Move the picture a little now and then (protects the screen)
        </label>
        <div className="row">
          <label className="check">
            <input type="checkbox" checked={display.night.enabled} onChange={(e) => set({ night: { ...display.night, enabled: e.target.checked } })} /> Darker at night
          </label>
          <input type="time" aria-label="Night from" value={display.night.from} onChange={(e) => set({ night: { ...display.night, from: e.target.value } })} />
          <span>to</span>
          <input type="time" aria-label="Night to" value={display.night.to} onChange={(e) => set({ night: { ...display.night, to: e.target.value } })} />
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
