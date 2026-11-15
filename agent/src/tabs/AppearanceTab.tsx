import { useEffect, useState } from "react";
import { t, tn, type DisplaySettings, type LayoutFile, type ValidationIssue } from "@decka/shared";
import { call } from "../ipc";
import Banner from "../ui/Banner";
import Button from "../ui/Button";
import Field from "../ui/Field";
import KeyPreview from "./KeyPreview";
import { applyScreenPreset, fracToPercent, minToSec, percentToFrac, presetLabel, screenPreset, secToMin, type ScreenPreset } from "../editor/screenPreset";
import ThemeCards from "./ThemeCards";

/** A sample key for the style cards (the label is in the language of the window). */
const sampleKey = () => ({ id: "sample", kind: "button", x: 0, y: 0, w: 1, h: 1, label: t("look.sample_key"), icon: "lucide:music", actionId: "sample" }) as const;

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
  if (!display) return <p>{t("common.loading")}</p>;

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
      setMessage(t("look.saved"));
      setIssues([]);
    } else {
      setIssues(result.issues);
      setMessage(null);
    }
  }

  return (
    <div className="stack">
      <h2>{t("tab.look")}</h2>

      <section className="card">
        <h3>{t("look.theme")}</h3>
        <ThemeCards value={display.theme ?? "amoled"} display={display} onPick={(theme) => set({ theme })} />
      </section>

      <section className="card">
        <h3>{t("look.key_style")}</h3>
        <div className="style-cards">
          {(["standard", "full"] as const).map((face) => (
            <button key={face} type="button" className={(display.keyFace ?? "standard") === face ? "style-card selected" : "style-card"} onClick={() => set({ keyFace: face })}>
              <KeyPreview item={sampleKey()} display={{ ...display, keyFace: face }} />
              <strong>{face === "standard" ? t("look.standard") : t("menu_item.big_icon")}</strong>
              <small className="muted">{face === "standard" ? t("look.standard_hint") : t("look.big_hint")}</small>
            </button>
          ))}
        </div>
        <label className="check">
          <input type="checkbox" checked={display.keyLabels !== false} onChange={(e) => set({ keyLabels: e.target.checked })} /> {t("look.show_names")}
        </label>
        {lowres > 0 && (
          <div className="row">
            <span className="muted">{t("look.lowres")}</span>
            <Button
              onClick={() =>
                void call("refresh_program_icons").then(async (r) => {
                  setLowres(await call("count_lowres_icons"));
                  setMessage(r.updated > 0 ? tn("look.sharpened", r.updated) : t("look.nothing_to_sharpen"));
                })
              }
            >
              {t("look.sharpen")}
            </Button>
          </div>
        )}
      </section>

      <section className="card">
        <h3>{t("look.accent")}</h3>
        <p className="muted">{t("look.accent_hint")}</p>
        <div className="row wrap">
          {ACCENTS.map((c) => (
            <button key={c} type="button" className={accent.toLowerCase() === c ? "swatch selected" : "swatch"} style={{ background: c }} aria-label={t("look.accent_swatch", { c })} onClick={() => set({ accent: c })} />
          ))}
          <label className="check">
            <input type="color" value={accent} onChange={(e) => set({ accent: e.target.value })} aria-label={t("look.custom_accent")} /> {t("look.custom")}
          </label>
        </div>
      </section>

      {display.theme === "glass" && (
        <section className="card">
          <h3>{t("look.wallpaper")}</h3>
          <p className="muted">{t("look.wallpaper_hint")}</p>
          <div className="row wrap">
            {WALLPAPERS.map((w) => (
              <label key={w} className="check">
                <input type="radio" name="wallpaper" checked={(display.wallpaper ?? "aurora") === w} onChange={() => set({ wallpaper: w })} /> {t(`wallpaper.${w}`)}
              </label>
            ))}
          </div>
        </section>
      )}

      <section className="card">
        <h3>{t("look.pages")}</h3>
        <label className="check">
          <input type="checkbox" checked={display.contextPages !== false} onChange={(e) => set({ contextPages: e.target.checked })} /> {t("look.context_pages")}
        </label>
        <p className="muted">{t("look.context_hint")}</p>
      </section>

      <section className="card">
        <h3>{t("look.screen_saving")}</h3>
        <div className="seg" role="group" aria-label={t("look.screen_saving")}>
          {(["off", "balanced", "maximum", "custom"] as ScreenPreset[]).map((p) => (
            <button key={p} type="button" className={(custom ? "custom" : preset) === p ? "on" : ""} onClick={() => (p === "custom" ? setCustom(true) : (setCustom(false), set(applyScreenPreset(display, p))))}>
              {presetLabel(p)}
            </button>
          ))}
        </div>
        <p className="muted">
          {preset === "balanced" && !custom && t("look.preset_balanced")}
          {preset === "maximum" && !custom && t("look.preset_maximum")}
          {preset === "off" && !custom && t("look.preset_off")}
          {(custom || preset === "custom") && t("look.preset_custom")}
        </p>
        {(custom || preset === "custom") && (
          <div className="grid-4">
            <Field label={t("look.dim_after")} hint={t("look.zero_never")}>
              <input type="number" min={0} step={0.5} value={secToMin(display.dimAfterSec)} onChange={(e) => set({ dimAfterSec: minToSec(Number(e.target.value)) })} />
            </Field>
            <Field label={t("look.brightness")} hint={t("look.brightness_hint")}>
              <input type="number" min={5} max={100} value={fracToPercent(display.dimLevel)} onChange={(e) => set({ dimLevel: percentToFrac(Number(e.target.value)) })} />
            </Field>
            <Field label={t("look.saver_after")} hint={t("look.zero_never")}>
              <input type="number" min={0} step={0.5} value={secToMin(display.sleepAfterSec)} onChange={(e) => set({ sleepAfterSec: minToSec(Number(e.target.value)) })} />
            </Field>
          </div>
        )}
        <label className="check">
          <input type="checkbox" checked={display.pixelShift} onChange={(e) => set({ pixelShift: e.target.checked })} /> {t("look.pixel_shift")}
        </label>
        <div className="row">
          <label className="check">
            <input type="checkbox" checked={display.night.enabled} onChange={(e) => set({ night: { ...display.night, enabled: e.target.checked } })} /> {t("look.night")}
          </label>
          <input type="time" aria-label={t("look.night_from")} value={display.night.from} onChange={(e) => set({ night: { ...display.night, from: e.target.value } })} />
          <span>{t("look.night_to_word")}</span>
          <input type="time" aria-label={t("look.night_to")} value={display.night.to} onChange={(e) => set({ night: { ...display.night, to: e.target.value } })} />
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
          {t("common.save")}
        </Button>
        {dirty && <span className="unsaved">{t("common.unsaved")}</span>}
      </div>
    </div>
  );
}
