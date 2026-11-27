import { useEffect, useState } from "react";
import { t, type LanguageSetting, type LayoutFile, type VoiceStatus } from "@decka/shared";
import logoUrl from "../../../docs/assets/decka-logo.svg";
import { call, onVoiceStatus } from "../ipc";
import { saveLanguage } from "../language";
import { languageSelectOptions } from "./LanguageOptions";
import Banner from "../ui/Banner";
import Button from "../ui/Button";
import Select from "../ui/Select";
import PhIcon from "../ui/PhIcon";
import { Group, PageHeader, Row } from "../ui/SettingsGroup";
import PairingTab from "./PairingTab";
import { formatHardware } from "./VoiceSection";

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

const DEFAULT_VOICE: VoiceStatus = {
  state: "off",
  hardware: { ramGb: 16 },
  model: "standard",
  downloadGb: 3.2,
  hotkey: ["ctrl", "alt", "space"],
  keepLoaded: false,
  unloadAfterMin: 10,
  speak: false,
  phoneButton: true,
  mics: [],
};

const MODEL_GB = {
  standard: "3.2",
  light: "1.8",
  "9b": "6.4",
} as const;

/** T31.4b first-run wizard: 3 steps (Phone -> Voice -> Done), Skip on every step. */
export default function FirstRun({
  onClose,
  startStep = 0,
  initialVoiceStatus,
}: {
  onClose: () => void;
  startStep?: number;
  initialVoiceStatus?: VoiceStatus;
}) {
  const [step, setStep] = useState(startStep);
  const [layout, setLayout] = useState<LayoutFile | null>(null);
  const [voice, setVoice] = useState<VoiceStatus>(initialVoiceStatus ?? DEFAULT_VOICE);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let active = true;
    call("get_layout").then(
      (l) => active && setLayout(l),
      (e) => active && setError(String(e)),
    );
    if (!initialVoiceStatus) {
      call("voice_status").then(
        (s) => active && setVoice(s),
        (e) => active && setError(String(e)),
      );
      let off: (() => void) | null = null;
      onVoiceStatus((s) => {
        if (active) setVoice(s);
      }).then(
        (unlisten) => {
          if (active) off = unlisten;
          else unlisten();
        },
        (e) => active && setError(String(e)),
      );
      return () => {
        active = false;
        off?.();
      };
    }
    return () => {
      active = false;
    };
  }, [initialVoiceStatus]);

  function finish() {
    markWizardDone();
    onClose();
  }

  function chooseLanguage(next: LanguageSetting) {
    setLayout((l) => (l ? { ...l, display: { ...l.display, language: next } } : l));
    void saveLanguage(next);
  }

  async function selectModel(model: "standard" | "light") {
    const before = voice;
    setError(null);
    setVoice({ ...voice, model, downloadGb: Number(MODEL_GB[model]) });
    try {
      const next = await call("voice_set", { model });
      setVoice(next);
    } catch (e) {
      setVoice(before);
      setError(String(e));
    }
  }

  async function runVoice(cmd: "voice_enable" | "voice_cancel") {
    setBusy(true);
    setError(null);
    try {
      const next = await call(cmd);
      setVoice(next);
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }

  const titles = [t("wizard.title_pair"), t("m31.wizard_voice_title"), t("m31.wizard_done_title")];
  const recommendedTier: "standard" | "light" = voice.suggestLight ? "light" : "standard";
  const downloadSize = voice.downloadGb ? String(voice.downloadGb) : MODEL_GB[voice.model];
  const phrases = [t("m31.wizard_phrase_1"), t("m31.wizard_phrase_2"), t("m31.wizard_phrase_3")];

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

        {step === 0 && (
          <div className="wizard-body">
            <PairingTab embedded />
          </div>
        )}

        {step === 1 && (
          <div className="wizard-body">
            <PageHeader icon="microphone" title={t("m31.wizard_voice_title")} subtitle={t("m31.wizard_voice_hint")} />
            <Group>
              <Row title={t("voice.title")} description={formatHardware(voice.hardware, voice.model)} icon="cpu" />
              {(["standard", "light"] as const).map((tier) => {
                const selected = voice.model === tier;
                const rec = tier === recommendedTier;
                return (
                  <Row
                    key={tier}
                    title={t(tier === "light" ? "voice.model_light" : "voice.model_standard")}
                    description={
                      rec
                        ? `${t("m31.wizard_voice_recommended")} · ~${MODEL_GB[tier]} GB`
                        : `~${MODEL_GB[tier]} GB`
                    }
                    icon="waveform"
                  >
                    <Button
                      variant={selected ? "primary" : undefined}
                      disabled={busy || voice.state === "downloading"}
                      onClick={() => void selectModel(tier)}
                    >
                      {tier === "light" ? t("voice.model_light_short") : t("voice.model_standard_short")}
                    </Button>
                  </Row>
                );
              })}
            </Group>

            <Group>
              {voice.state === "downloading" ? (
                <div style={{ padding: "14px 0" }}>
                  <p>
                    {t("voice.downloading", {
                      done: voice.progress ? (voice.progress.done / 1e9).toFixed(1) : "0.0",
                      total: voice.progress ? (voice.progress.total / 1e9).toFixed(1) : downloadSize,
                    })}
                  </p>
                  <div className="progress-bar">
                    <div
                      className="progress-fill"
                      style={{
                        width: `${
                          voice.progress && voice.progress.total > 0
                            ? Math.min(100, Math.round((voice.progress.done / voice.progress.total) * 100))
                            : 0
                        }%`,
                      }}
                    />
                  </div>
                  <div style={{ marginTop: 12 }}>
                    <Button disabled={busy} onClick={() => void runVoice("voice_cancel")}>
                      {t("common.cancel")}
                    </Button>
                  </div>
                </div>
              ) : voice.state === "loading" ? (
                <div style={{ padding: "14px 0" }}>
                  <p className="muted">{t("voice.loading")}</p>
                </div>
              ) : voice.state === "ready" ? (
                <div style={{ padding: "14px 0" }}>
                  <Banner kind="success">{t("m31.wizard_voice_ready")}</Banner>
                </div>
              ) : (
                <div style={{ padding: "14px 0" }}>
                  {voice.state === "error" && voice.error && <Banner kind="error">{voice.error}</Banner>}
                  <Button variant="primary" disabled={busy} onClick={() => void runVoice("voice_enable")}>
                    {t("voice.turn_on", { size: downloadSize })}
                  </Button>
                  <p className="muted" style={{ fontSize: 12, marginTop: 8, marginBottom: 0 }}>
                    {t("voice.privacy")}
                  </p>
                </div>
              )}
            </Group>
          </div>
        )}

        {step === 2 && (
          <div className="wizard-body">
            <PageHeader icon="check-circle" title={t("m31.wizard_done_title")} subtitle={t("m31.wizard_done_hint")} />
            <Group>
              {phrases.map((phrase, idx) => (
                <Row key={idx} icon="chats-circle" title={`«${phrase}»`} />
              ))}
            </Group>
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
          {step < 2 ? (
            <Button variant="primary" disabled={busy} onClick={() => setStep(step + 1)}>
              {t("tips.next")}
            </Button>
          ) : (
            <Button variant="primary" disabled={busy} onClick={finish}>
              {t("wizard.finish")}
            </Button>
          )}
        </div>
      </div>
    </div>
  );
}
