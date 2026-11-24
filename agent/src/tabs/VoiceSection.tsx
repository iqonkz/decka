import { useEffect, useState } from "react";
import { t, type VoiceStatus } from "@decka/shared";
import { call, onVoiceStatus } from "../ipc";
import Button from "../ui/Button";
import Banner from "../ui/Banner";
import Field from "../ui/Field";
import ChordField from "../editor/ChordField";
import { formatChord } from "../editor/addButton";
import VoiceHelp from "./VoiceHelp";
import DictationBlock from "./DictationBlock";
import MemoryBlock from "./MemoryBlock";

const DEFAULT_STATUS: VoiceStatus = {
  state: "off",
  hardware: { ramGb: 16 },
  model: "standard",
  downloadGb: 3.2,
  hotkey: ["ctrl", "alt", "space"],
  keepLoaded: false,
  speak: false,
  phoneButton: true,
  mics: [],
};

export function formatHardware(hw: { gpu?: string; vramGb?: number; ramGb: number }, model: "standard" | "light"): string {
  const parts: string[] = [];
  if (hw.gpu) {
    if (hw.vramGb != null) parts.push(`${hw.gpu} (${Math.round(hw.vramGb)} GB)`);
    else parts.push(hw.gpu);
  }
  parts.push(`${hw.ramGb.toFixed(1)} GB RAM`);
  const modelName = t(model === "light" ? "voice.model_light_short" : "voice.model_standard_short");
  return t("voice.hw_summary", { details: parts.join(", "), model: modelName });
}

/** Settings window: More → Voice (DATA_MODEL.md 12.7). */
export default function VoiceSection() {
  const [status, setStatus] = useState<VoiceStatus>(DEFAULT_STATUS);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let active = true;
    call("voice_status").then(
      (s) => active && setStatus(s),
      () => {},
    );

    let off: (() => void) | null = null;
    onVoiceStatus((s) => {
      if (active) setStatus(s);
    }).then(
      (unlisten) => {
        if (active) off = unlisten;
        else unlisten();
      },
      () => {},
    );

    return () => {
      active = false;
      off?.();
    };
  }, []);

  async function run(cmd: "voice_enable" | "voice_cancel" | "voice_remove") {
    setBusy(true);
    setError(null);
    try {
      const next = await call(cmd);
      setStatus(next);
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }

  async function setConfig(args: {
    model?: "standard" | "light";
    hotkey?: string[];
    keepLoaded?: boolean;
    speak?: boolean;
    phoneButton?: boolean;
    mic?: string;
  }) {
    setError(null);
    try {
      const next = await call("voice_set", args);
      setStatus(next);
    } catch (e) {
      setError(String(e));
    }
  }

  return (
    <div className="stack">
      <h2>{t("more.voice")}</h2>
      {error && <Banner kind="error">{error}</Banner>}

      {status.state === "off" && (
        <section className="card">
          <h3>{t("voice.title")}</h3>
          <p>{t("voice.desc")}</p>
          <p className="muted">{formatHardware(status.hardware, status.model)}</p>
          <div>
            <Button variant="primary" disabled={busy} onClick={() => void run("voice_enable")}>
              {t("voice.turn_on", { size: status.downloadGb || (status.model === "light" ? "1.8" : "3.2") })}
            </Button>
            <p className="muted" style={{ fontSize: 12, marginTop: 6, marginBottom: 0 }}>
              {t("voice.privacy")}
            </p>
          </div>
        </section>
      )}

      {status.state === "downloading" && (
        <section className="card">
          <h3>{t("voice.title")}</h3>
          <p>
            {t("voice.downloading", {
              done: status.progress ? (status.progress.done / 1e9).toFixed(1) : "0.0",
              total: status.progress ? (status.progress.total / 1e9).toFixed(1) : String(status.downloadGb || 3.2),
            })}
          </p>
          <div className="progress-bar">
            <div
              className="progress-fill"
              style={{
                width: `${
                  status.progress && status.progress.total > 0
                    ? Math.min(100, Math.round((status.progress.done / status.progress.total) * 100))
                    : 0
                }%`,
              }}
            />
          </div>
          <div style={{ marginTop: 10 }}>
            <Button disabled={busy} onClick={() => void run("voice_cancel")}>
              {t("common.cancel")}
            </Button>
          </div>
        </section>
      )}

      {status.state === "loading" && (
        <section className="card">
          <h3>{t("voice.title")}</h3>
          <p className="muted">{t("voice.loading")}</p>
        </section>
      )}

      {status.state === "error" && (
        <section className="card">
          <h3>{t("voice.title")}</h3>
          <Banner kind="error">{status.error ?? ""}</Banner>
          <div style={{ marginTop: 10 }}>
            <Button variant="primary" disabled={busy} onClick={() => void run("voice_enable")}>
              {t("voice.turn_on", { size: status.downloadGb || "3.2" })}
            </Button>
          </div>
        </section>
      )}

      {status.state === "ready" && (
        <section className="card">
          <div className="row" style={{ marginBottom: 12 }}>
            <h3>{t("voice.title")}</h3>
            <span className="grow" />
            <span className="ok-text">{t("voice.ready", { hotkey: formatChord(status.hotkey) })}</span>
          </div>

          <div className="stack" style={{ gap: 12 }}>
            <Field label={t("voice.hotkey")}>
              <ChordField keys={status.hotkey} onChange={(keys) => void setConfig({ hotkey: keys })} />
            </Field>

            <Field label={t("voice.mic")}>
              <select value={status.mic ?? ""} onChange={(e) => void setConfig({ mic: e.target.value })}>
                <option value="">{t("voice.mic_default")}</option>
                {status.mics.map((m) => (
                  <option key={m} value={m}>
                    {m}
                  </option>
                ))}
              </select>
            </Field>

            <label className="check">
              <input
                type="checkbox"
                checked={status.keepLoaded}
                onChange={(e) => void setConfig({ keepLoaded: e.target.checked })}
              />
              <span>
                <strong>{t("voice.keep_loaded")}</strong>
                <span className="muted" style={{ display: "block", fontSize: 12 }}>
                  {t("voice.keep_loaded_hint")}
                </span>
              </span>
            </label>

            {/* "Speak the answers" is hidden until the spoken answers exist (DATA_MODEL.md 12.10, "later"). */}

            <label className="check">
              <input
                type="checkbox"
                checked={status.phoneButton}
                onChange={(e) => void setConfig({ phoneButton: e.target.checked })}
              />
              <span>{t("voice.phone_button")}</span>
            </label>

            <details style={{ marginTop: 8 }}>
              <summary style={{ cursor: "pointer", fontWeight: 600 }}>{t("voice.advanced")}</summary>
              <div className="stack" style={{ marginTop: 10, paddingInlineStart: 12 }}>
                <Field label={t("voice.model")}>
                  <select
                    value={status.model}
                    onChange={(e) => void setConfig({ model: e.target.value as "standard" | "light" })}
                  >
                    <option value="standard">{t("voice.model_standard")}</option>
                    <option value="light">{t("voice.model_light")}</option>
                  </select>
                </Field>
                <p className="muted" style={{ fontSize: 12, margin: 0 }}>
                  {t("voice.models_info", {
                    llm: status.model === "light" ? "Qwen3.5 2B" : "Qwen3.5 4B",
                  })}
                </p>
              </div>
            </details>

            <div style={{ marginTop: 16 }}>
              <Button disabled={busy} onClick={() => void run("voice_remove")}>
                {t("voice.remove")}
              </Button>
            </div>
          </div>
        </section>
      )}
      <DictationBlock />
      <MemoryBlock />
      <VoiceHelp />
    </div>
  );
}
