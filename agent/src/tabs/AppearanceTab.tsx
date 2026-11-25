import { useEffect, useState } from "react";
import { t, tn, type DisplaySettings, type LayoutFile, type ValidationIssue, type WindowTheme } from "@decka/shared";
import { call } from "../ipc";
import Banner from "../ui/Banner";
import Button from "../ui/Button";
import Field from "../ui/Field";
import KeyPreview from "./KeyPreview";
import { applyScreenPreset, fracToPercent, minToSec, percentToFrac, presetLabel, screenPreset, secToMin, type ScreenPreset } from "../editor/screenPreset";
import OrientationSection from "./OrientationSection";
import ThemeCards from "./ThemeCards";
import { Group, PageHeader, Row, SectionHeader } from "../ui/SettingsGroup";
import Switch from "../ui/Switch";
import Segmented from "../ui/Segmented";
import PhIcon from "../ui/PhIcon";
import ColorPicker from "../ui/ColorPicker";
import RadioGroup from "../ui/RadioGroup";
import TimeField from "../ui/TimeField";
import { useAutosave } from "../ui/useAutosave";
import { applyWindowTheme } from "../windowTheme";

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

  useAutosave({
    value: display,
    enabled: display !== null && saved !== "",
    debounceMs: 400,
    onSave: async (currentDisplay) => {
      if (!currentDisplay) return { ok: true };
      const fresh: LayoutFile = await call("get_layout");
      const result = await call("save_layout", { layout: { ...fresh, display: currentDisplay } });
      if (result && result.ok) {
        setSaved(JSON.stringify(currentDisplay));
        setIssues([]);
        return { ok: true };
      }
      setIssues(result?.issues ?? []);
      return { ok: false, error: result?.issues?.[0]?.message };
    },
    onRollback: (savedDisplay) => {
      if (savedDisplay) setDisplay(savedDisplay);
    },
  });

  if (error) return <Banner kind="error">{error}</Banner>;
  if (!display) return <p>{t("common.loading")}</p>;

  const set = (patch: Partial<DisplaySettings>) => {
    setDisplay({ ...display, ...patch });
    setMessage(null);
    setIssues([]);
  };
  const preset = screenPreset(display);
  const accent = display.accent ?? "#7c5cff";

  const screenPresets: ScreenPreset[] = ["off", "balanced", "maximum", "custom"];

  return (
    <div className="page-container">
      <PageHeader icon="palette" title={t("tab.look")} />

      <SectionHeader id="look-window-theme" title={t("look.window_theme")} />
      <Group>
        <Row title={t("look.window_theme_row")}>
          <Segmented
            options={[
              { value: "system", label: t("theme.system") },
              { value: "light", label: t("theme.light") },
              { value: "dark", label: t("theme.dark") },
            ]}
            value={display.windowTheme ?? "system"}
            onChange={(val) => {
              const next = val as WindowTheme;
              set({ windowTheme: next });
              applyWindowTheme(next);
            }}
          />
        </Row>
      </Group>

      <SectionHeader id="section-look-theme" title={t("look.theme")} />
      <Group>
        <div style={{ padding: "14px 0" }}>
          <ThemeCards value={display.theme ?? "amoled"} display={display} onPick={(theme) => set({ theme })} />
        </div>
      </Group>

      <SectionHeader title={t("orientation.title")} />
      <Group>
        <div style={{ padding: "14px 0" }}>
          <OrientationSection
            orientation={display.orientation ?? "landscape"}
            onChanged={(layout) => {
              setDisplay(layout.display);
              setSaved(JSON.stringify(layout.display));
            }}
          />
        </div>
      </Group>

      <SectionHeader id="look-key-style" title={t("look.key_style")} />
      <Group>
        <div className="style-cards" style={{ padding: "14px 0 0" }}>
          {(["standard", "full"] as const).map((face) => {
            const isSelected = (display.keyFace ?? "standard") === face;
            return (
              <button
                key={face}
                type="button"
                className={isSelected ? "style-card selected" : "style-card"}
                onClick={() => set({ keyFace: face })}
              >
                {isSelected && (
                  <span className="selection-badge">
                    <PhIcon name="check-circle" size={20} weight="fill" color="var(--accent)" />
                  </span>
                )}
                <KeyPreview item={sampleKey()} display={{ ...display, keyFace: face }} />
                <strong>{face === "standard" ? t("look.standard") : t("menu_item.big_icon")}</strong>
                <small className="muted">{face === "standard" ? t("look.standard_hint") : t("look.big_hint")}</small>
              </button>
            );
          })}
        </div>
        <Row title={t("look.show_names")}>
          <Switch checked={display.keyLabels !== false} onChange={(checked) => set({ keyLabels: checked })} />
        </Row>
        {lowres > 0 && (
          <Row title={t("look.lowres")}>
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
          </Row>
        )}
      </Group>

      <SectionHeader id="section-look-accent" title={t("look.accent")} subtitle={t("look.accent_hint")} />
      <Group>
        <div style={{ padding: "14px 0" }}>
          <ColorPicker value={accent} onChange={(nextAccent) => set({ accent: nextAccent })} />
        </div>
      </Group>

      {display.theme === "glass" && (
        <>
          <SectionHeader id="section-look-wallpaper" title={t("look.wallpaper")} subtitle={t("look.wallpaper_hint")} />
          <Group>
            <div style={{ padding: "14px 0" }}>
              <RadioGroup
                aria-label={t("look.wallpaper")}
                value={display.wallpaper ?? "aurora"}
                options={WALLPAPERS.map((w) => ({ value: w, label: t(`wallpaper.${w}`) }))}
                onChange={(w) => set({ wallpaper: w })}
              />
            </div>
          </Group>
        </>
      )}

      <SectionHeader id="look-pages" title={t("look.pages")} />
      <Group>
        <Row title={t("look.context_pages")} description={t("look.context_hint")}>
          <Switch checked={display.contextPages !== false} onChange={(checked) => set({ contextPages: checked })} />
        </Row>
      </Group>

      <SectionHeader id="section-look-screen-saving" title={t("look.screen_saving")} />
      <Group>
        <div style={{ padding: "14px 0 8px" }}>
          <Segmented
            options={screenPresets.map((p) => ({ value: p, label: presetLabel(p) }))}
            value={custom ? "custom" : preset}
            onChange={(val) => {
              if (val === "custom") {
                setCustom(true);
              } else {
                setCustom(false);
                set(applyScreenPreset(display, val));
              }
            }}
          />
          <p className="muted" style={{ margin: "10px 0 0" }}>
            {preset === "balanced" && !custom && t("look.preset_balanced")}
            {preset === "maximum" && !custom && t("look.preset_maximum")}
            {preset === "off" && !custom && t("look.preset_off")}
            {(custom || preset === "custom") && t("look.preset_custom")}
          </p>
          {(custom || preset === "custom") && (
            <div className="grid-4" style={{ marginTop: 12 }}>
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
        </div>
        <Row title={t("look.pixel_shift")}>
          <Switch checked={display.pixelShift} onChange={(checked) => set({ pixelShift: checked })} />
        </Row>
        <Row id="section-look-night" title={t("look.night")}>
          <div className="row">
            <Switch checked={display.night.enabled} onChange={(checked) => set({ night: { ...display.night, enabled: checked } })} />
            <TimeField aria-label={t("look.night_from")} value={display.night.from} onChange={(from) => set({ night: { ...display.night, from } })} />
            <span>{t("look.night_to_word")}</span>
            <TimeField aria-label={t("look.night_to")} value={display.night.to} onChange={(to) => set({ night: { ...display.night, to } })} />
          </div>
        </Row>
      </Group>

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
    </div>
  );
}
