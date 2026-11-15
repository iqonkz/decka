import { useEffect, useMemo, useState } from "react";
import type { InstalledApp, LayoutFile } from "@decka/shared";
import { call } from "../ipc";
import Banner from "../ui/Banner";
import Button from "../ui/Button";
import { presetActions } from "../editor/addButton";
import { DEFAULT_PRESETS, planWizard, type WizardPick } from "../editor/wizardPlan";
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

/** The first-run guide (DESIGN.md 4.4): pair the phone, pick a look, add programs. Every step can be skipped. */
export default function FirstRun({ onClose, startStep = 0 }: { onClose: () => void; startStep?: number }) {
  const [step, setStep] = useState(startStep);
  const [presets, setPresets] = useState<Set<string>>(new Set(DEFAULT_PRESETS));
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
      const result = await call("save_layout", { layout: { ...fresh, display: { ...fresh.display, ...layout.display } } });
      if (!result.ok) setError(result.issues[0]?.message ?? "Cannot save the look");
      else setStep(2);
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }

  async function addApps() {
    const chosen = (apps ?? []).filter((a) => picked.has(key(a)));
    if (chosen.length === 0 && presets.size === 0) return finish();
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
      const plan = planWizard(currentLayout, currentActions, picks, [...presets]);
      const savedActions = await call("save_actions", { actions: plan.actions });
      if (!savedActions.ok) throw new Error(savedActions.issues[0]?.message ?? "Cannot save the actions");
      const savedLayout = await call("save_layout", { layout: plan.layout });
      if (!savedLayout.ok) throw new Error(savedLayout.issues[0]?.message ?? "Cannot save the layout");
      setMessage(plan.skipped > 0 ? `Added ${plan.placed}. ${plan.skipped} more did not fit on the first page.` : `Added ${plan.placed}.`);
      window.setTimeout(finish, plan.skipped > 0 ? 2500 : 900);
    } catch (e) {
      setError(String(e instanceof Error ? e.message : e));
    } finally {
      setBusy(false);
    }
  }

  const titles = ["Pair your phone", "Pick a look", "Add your apps"];
  return (
    <div className="wizard" role="dialog" aria-label="Welcome to Decka">
      <div className="wizard-card">
        <p className="muted wizard-step">
          Step {step + 1} of 3 · {titles[step]}
        </p>
        {error && <Banner kind="error">{error}</Banner>}
        {message && <Banner kind="success">{message}</Banner>}

        {step === 0 && <PairingTab />}

        {step === 1 && layout && (
          <>
            <h2>Pick a look</h2>
            <p className="muted">You can change it any time in Appearance.</p>
            <ThemeCards value={layout.display.theme ?? "amoled"} display={layout.display} onPick={(theme) => setLayout({ ...layout, display: { ...layout.display, theme } })} />
          </>
        )}

        {step === 2 && (
          <>
            <h2>Add your apps</h2>
            <p className="muted">Tick what you want as buttons on the first page. Nothing already there is changed.</p>
            <h3>Media &amp; system</h3>
            <div className="wizard-presets">
              {presetActions().map((p) => (
                <label key={p.id} className="check">
                  <input
                    type="checkbox"
                    checked={presets.has(p.id)}
                    onChange={(e) =>
                      setPresets((prev) => {
                        const next = new Set(prev);
                        if (e.target.checked) next.add(p.id);
                        else next.delete(p.id);
                        return next;
                      })
                    }
                  />{" "}
                  {p.label}
                </label>
              ))}
            </div>
            <h3>Programs</h3>
            <input aria-label="Search" placeholder="Search" value={query} onChange={(e) => setQuery(e.target.value)} />
            {apps === null && !error && <p className="muted">Loading installed apps…</p>}
            <div className="app-list">
              {shown.map((a) => (
                <label key={key(a)} className="app-row">
                  <input
                    type="checkbox"
                    checked={picked.has(key(a))}
                    disabled={!picked.has(key(a)) && picked.size >= MAX_PICKS}
                    onChange={(e) =>
                      setPicked((prev) => {
                        const next = new Set(prev);
                        if (e.target.checked) next.add(key(a));
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
            <p className="muted">{picked.size} programs and {presets.size} actions selected (up to {MAX_PICKS} programs).</p>
          </>
        )}

        <div className="row wizard-actions">
          <Button onClick={() => setStep((s) => Math.max(0, s - 1))} disabled={step === 0 || busy}>
            Back
          </Button>
          <span style={{ flex: 1 }} />
          <Button onClick={() => (step < 2 ? setStep(step + 1) : finish())} disabled={busy}>
            Skip
          </Button>
          {step === 0 && (
            <Button variant="primary" onClick={() => setStep(1)}>
              Next
            </Button>
          )}
          {step === 1 && (
            <Button variant="primary" disabled={busy} onClick={() => void saveLook()}>
              Next
            </Button>
          )}
          {step === 2 && (
            <Button variant="primary" disabled={busy} onClick={() => void addApps()}>
              {picked.size + presets.size > 0 ? "Add and finish" : "Finish"}
            </Button>
          )}
        </div>
      </div>
    </div>
  );
}
