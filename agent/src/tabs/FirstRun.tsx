import { useEffect, useMemo, useState } from "react";
import { SETS, getLanguage, pickText, insertSet, rotateLayout, t, tn, type ActionsFile, type InstalledApp, type LanguageSetting, type LayoutFile } from "@decka/shared";
import logoUrl from "../../../docs/assets/decka-logo.svg";
import { call } from "../ipc";
import { saveLanguage } from "../language";
import { languageSelectOptions } from "./LanguageOptions";
import Banner from "../ui/Banner";
import Button from "../ui/Button";
import Checkbox from "../ui/Checkbox";
import Select from "../ui/Select";
import EntryIcon from "../ui/EntryIcon";
import PhIcon from "../ui/PhIcon";
import { Group, PageHeader, SectionHeader } from "../ui/SettingsGroup";
import { insertContext, itemsOfSet } from "../editor/catalogUi";
import { DEFAULT_SETS, planWizard, type WizardPick } from "../editor/wizardPlan";
import PairingTab from "./PairingTab";
import ThemeCards from "./ThemeCards";

export const WIZARD_KEY = "decka.wizard.done.v1";

/** The guide is shown once: the flag lives in this window's storage (it may be blocked, then it simply shows again). */
export function wizardDone(storage: Pick<Storage, "getItem"> | undefined = safeStorage()): boolean {
  try {
    return storage?.getItem(WIZARD_KEY) === "1";
  } catch {
    return false;
  }
}

export function markWizardDone(storage: Pick<Storage, "setItem"> | undefined = safeStorage()): void {
  try {
    storage?.setItem(WIZARD_KEY, "1");
  } catch {
    // storage blocked: nothing to remember it with
  }
}

function safeStorage(): Storage | undefined {
  try {
    return window.localStorage;
  } catch {
    return undefined;
  }
}

const MAX_PICKS = 20;

/** The first-run guide (DESIGN.md 4.4, 4.7, 4.14-4.15): pair the phone, pick a look, add programs. */
export default function FirstRun({ onClose, startStep = 0 }: { onClose: () => void; startStep?: number }) {
  const [step, setStep] = useState(startStep);
  const [sets, setSets] = useState<Set<string>>(new Set(DEFAULT_SETS));
  // OBS is running, or installed (a Start menu shortcut): the Streaming set is offered only then (DESIGN.md 4.8).
  const [obsFound, setObsFound] = useState(false);
  const [layout, setLayout] = useState<LayoutFile | null>(null);
  const [apps, setApps] = useState<InstalledApp[] | null>(null);
  const [picked, setPicked] = useState<Set<string>>(new Set());
  const [query, setQuery] = useState("");
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    call("get_layout").then(setLayout, (e) => setError(String(e)));
  }, []);
  useEffect(() => {
    if (step !== 2 || apps !== null) return;
    call("list_installed_apps").then(setApps, (e) => setError(String(e)));
  }, [step, apps]);
  useEffect(() => {
    if (step !== 2) return;
    void call("list_window_apps").then((running) => setObsFound((was) => was || running.includes("obs64.exe")), () => {});
  }, [step]);
  useEffect(() => {
    if (apps?.some((a) => /\bobs studio\b/i.test(a.name))) setObsFound(true);
  }, [apps]);

  const shown = useMemo(() => (apps ?? []).filter((a) => query.trim() === "" || a.name.toLowerCase().includes(query.trim().toLowerCase())), [apps, query]);
  const key = (a: InstalledApp) => `${a.program}|${a.args.join(" ")}`;

  function finish() {
    markWizardDone();
    onClose();
  }

  async function saveLook() {
    if (!layout) return setStep(2);
    setBusy(true);
    try {
      const fresh = await call("get_layout");
      let next: LayoutFile = { ...fresh, display: { ...fresh.display, ...layout.display } };
      // A different orientation turns the whole deck (grid and pages), not only the setting (DATA_MODEL.md 3.7).
      const wanted = layout.display.orientation ?? "landscape";
      if (wanted !== (fresh.display.orientation ?? "landscape")) {
        const turned = rotateLayout(fresh, wanted);
        if (!turned.ok) return setError(turned.error);
        next = { ...turned.layout, display: { ...turned.layout.display, ...layout.display, orientation: wanted } };
      }
      const result = await call("save_layout", { layout: next });
      if (!result.ok) setError(result.issues[0]?.message ?? t("wizard.cannot_save_look"));
      else setStep(2);
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }

  async function addApps() {
    const chosen = (apps ?? []).filter((a) => picked.has(key(a)));
    const chosenSets = SETS.filter((s) => sets.has(s.id) && (s.requires !== "obs" || obsFound));
    if (chosen.length === 0 && chosenSets.length === 0) return finish();
    setBusy(true);
    setError(null);
    try {
      const [currentLayout, currentActions] = await Promise.all([call("get_layout"), call("get_actions")]);
      const picks: WizardPick[] = await Promise.all(
        chosen.map(async (app) => {
          const r = await call("icon_from_program", { path: app.program }).catch(() => null);
          return r && "icon" in r ? { app, icon: r.icon } : { app };
        }),
      );
      // The programs first, then the ticked sets onto the first page (DESIGN.md 4.8); a set that does not fit goes to a page of its own.
      const plan = planWizard(currentLayout, currentActions, picks);
      let planned: { layout: LayoutFile; actions: ActionsFile; placed: number; skipped: number } = plan;
      for (const set of chosenSets) {
        const first = planned.layout.pages[0];
        if (!first) break;
        const ctx = await insertContext(itemsOfSet(set), planned.layout, null);
        const r = insertSet(planned.layout, planned.actions, first.id, set, ctx);
        if (r.ok) planned = { layout: r.layout, actions: r.actions, placed: planned.placed + r.added.length, skipped: planned.skipped + r.skipped.length };
      }
      const savedActions = await call("save_actions", { actions: planned.actions });
      if (!savedActions.ok) throw new Error(savedActions.issues[0]?.message ?? t("wizard.cannot_save_actions"));
      const savedLayout = await call("save_layout", { layout: planned.layout });
      if (!savedLayout.ok) throw new Error(savedLayout.issues[0]?.message ?? t("wizard.cannot_save_layout"));
      setMessage(planned.skipped > 0 ? t("wizard.added_skipped", { placed: planned.placed, skipped: planned.skipped }) : t("wizard.added", { placed: planned.placed }));
      window.setTimeout(finish, planned.skipped > 0 ? 2500 : 900);
    } catch (e) {
      setError(String(e instanceof Error ? e.message : e));
    } finally {
      setBusy(false);
    }
  }

  const titles = [t("wizard.title_pair"), t("wizard.title_look"), t("wizard.title_apps")];
  // The language switch applies at once; the guide's copy of the display settings follows so that `Next` does not undo it.
  function chooseLanguage(next: LanguageSetting) {
    setLayout((l) => (l ? { ...l, display: { ...l.display, language: next } } : l));
    void saveLanguage(next);
  }
  return (
    <div className="wizard" role="dialog" aria-label={t("wizard.welcome")}>
      <div className="wizard-card">
        <div className="wizard-topbar">
          <div className="wizard-steps-bar" aria-hidden="true">
            {[0, 1, 2].map((n) => (
              <span
                key={n}
                className={`wizard-step-pill${n === step ? " active" : n < step ? " done" : ""}`}
              />
            ))}
          </div>
          <span className="wizard-step-label">{t("wizard.step", { n: step + 1, title: titles[step] ?? "" })}</span>
          {step === 0 && (
            <div className="wizard-lang">
              <Select
                aria-label={t("settings.language")}
                value={layout?.display.language ?? "auto"}
                options={languageSelectOptions()}
                onChange={(val) => chooseLanguage(val as LanguageSetting)}
              />
            </div>
          )}
        </div>

        {step === 0 && (
          <div className="wizard-hero">
            <img className="logo-wizard" src={logoUrl} width={72} height={72} alt="Decka" />
            <PageHeader title={t("wizard.welcome")} subtitle={t("wizard.title_pair")} />
          </div>
        )}

        {error && <Banner kind="error">{error}</Banner>}
        {message && <Banner kind="success">{message}</Banner>}

        {step === 0 && (
          <div className="wizard-body">
            <PairingTab embedded />
          </div>
        )}

        {step === 1 && (
          <div className="wizard-body">
            <PageHeader icon="palette" title={t("wizard.title_look")} subtitle={t("wizard.look_hint")} />
            {layout && (
              <>
                <ThemeCards value={layout.display.theme ?? "amoled"} display={layout.display} onPick={(theme) => setLayout({ ...layout, display: { ...layout.display, theme } })} />
                <SectionHeader title={t("orientation.title")} />
                <div className="orientation-cards small" role="radiogroup" aria-label={t("orientation.title")}>
                  {(["landscape", "portrait"] as const).map((o) => {
                    const selected = (layout.display.orientation ?? "landscape") === o;
                    return (
                      <button
                        key={o}
                        type="button"
                        role="radio"
                        aria-checked={selected}
                        className={selected ? "style-card selected" : "style-card"}
                        onClick={() => setLayout({ ...layout, display: { ...layout.display, orientation: o } })}
                      >
                        {selected && (
                          <span className="selection-badge">
                            <PhIcon name="check-circle" size={20} weight="fill" color="var(--accent)" />
                          </span>
                        )}
                        <svg className="phone-silhouette" width={o === "portrait" ? 18 : 30} height={o === "portrait" ? 30 : 18} viewBox={o === "portrait" ? "0 0 28 48" : "0 0 48 28"} aria-hidden="true">
                          <rect x="1.5" y="1.5" width={o === "portrait" ? 25 : 45} height={o === "portrait" ? 45 : 25} rx="4" fill="none" stroke="currentColor" strokeWidth="3" />
                        </svg>
                        <strong>{t(`orientation.${o}`)}</strong>
                      </button>
                    );
                  })}
                </div>
              </>
            )}
          </div>
        )}

        {step === 2 && (
          <div className="wizard-body">
            <PageHeader icon="squares-four" title={t("wizard.title_apps")} subtitle={t("wizard.apps_hint")} />
            <SectionHeader title={t("wizard.sets")} subtitle={t("wizard.sets_hint")} />
            <div className="wizard-sets">
              {SETS.filter((s) => s.requires !== "obs" || obsFound).map((s) => {
                const isSelected = sets.has(s.id);
                return (
                  <label key={s.id} className={isSelected ? "set-card selected" : "set-card"}>
                    {isSelected && (
                      <span className="selection-badge">
                        <PhIcon name="check-circle" size={18} weight="fill" color="var(--accent)" />
                      </span>
                    )}
                    <span className="set-head">
                      <Checkbox
                        checked={isSelected}
                        aria-label={pickText(s.name, getLanguage())}
                        onChange={(on) =>
                          setSets((prev) => {
                            const next = new Set(prev);
                            if (on) next.add(s.id);
                            else next.delete(s.id);
                            return next;
                          })
                        }
                      />
                      <EntryIcon icon={s.icon} />
                      <strong>{pickText(s.name, getLanguage())}</strong>
                    </span>
                    <small className="muted">{pickText(s.description, getLanguage())}</small>
                  </label>
                );
              })}
            </div>
            <SectionHeader title={t("picker.programs")} />
            <div className="integrations-search wizard-search">
              <PhIcon name="magnifying-glass" size={16} />
              <input
                type="search"
                aria-label={t("wizard.search")}
                placeholder={t("wizard.search")}
                value={query}
                onChange={(e) => setQuery(e.target.value)}
              />
            </div>
            {apps === null && !error && <p className="muted">{t("picker.loading_apps")}</p>}
            <Group className="wizard-apps-group">
              <div className="app-list">
                {shown.map((a) => (
                  <label key={key(a)} className="app-row">
                    <Checkbox
                      checked={picked.has(key(a))}
                      aria-label={a.name}
                      disabled={!picked.has(key(a)) && picked.size >= MAX_PICKS}
                      onChange={(on) =>
                        setPicked((prev) => {
                          const next = new Set(prev);
                          if (on) next.add(key(a));
                          else next.delete(key(a));
                          return next;
                        })
                      }
                    />
                    {a.icon ? <img src={a.icon} width={32} height={32} alt="" /> : <span className="app-noicon" />}
                    <span className="app-text">
                      <strong>{a.name}</strong>
                      <small className="muted">{a.program}</small>
                    </span>
                  </label>
                ))}
              </div>
            </Group>
            <p className="muted">{t("wizard.selected", { programs: tn("wizard.programs", picked.size), actions: tn("wizard.actions", sets.size), max: MAX_PICKS })}</p>
          </div>
        )}

        <div className="row wizard-actions">
          <Button onClick={() => setStep((s) => Math.max(0, s - 1))} disabled={step === 0 || busy}>
            {t("picker.back")}
          </Button>
          <span style={{ flex: 1 }} />
          <Button onClick={() => (step < 2 ? setStep(step + 1) : finish())} disabled={busy}>
            {t("wizard.skip")}
          </Button>
          {step === 0 && (
            <Button variant="primary" onClick={() => setStep(1)}>
              {t("tips.next")}
            </Button>
          )}
          {step === 1 && (
            <Button variant="primary" disabled={busy} onClick={() => void saveLook()}>
              {t("tips.next")}
            </Button>
          )}
          {step === 2 && (
            <Button variant="primary" disabled={busy} onClick={() => void addApps()}>
              {picked.size + sets.size > 0 ? t("wizard.add_finish") : t("wizard.finish")}
            </Button>
          )}
        </div>
      </div>
    </div>
  );
}
