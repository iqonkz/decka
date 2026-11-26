import { useEffect, useState } from "react";
import { t, type IslandSettings, type VoiceStatus } from "@decka/shared";
import type { SavedGuide, VoiceModelTier, WorkMode } from "@decka/ui";
import { call, onVoiceStatus } from "../ipc";
import Button from "../ui/Button";
import Banner from "../ui/Banner";
import Modal from "../ui/Modal";
import ChordField from "../editor/ChordField";
import { formatChord } from "../editor/addButton";
import VoiceCommands from "./VoiceCommands";
import { loadVoiceExtras, saveDuckMusic, saveIsland } from "./voiceExtras";
import DictationBlock from "./DictationBlock";
import MemoryBlock from "./MemoryBlock";
import GuidesBlock from "./GuidesBlock";
import ModesBlock from "./ModesBlock";
import VoiceCheckPanel from "./VoiceCheckPanel";
import { voiceErrorCopy, type VoiceCheckStep } from "./voiceM28Types";
import { Group, PageHeader, Row, SectionHeader } from "../ui/SettingsGroup";
import Disclosure from "../ui/Disclosure";
import Select from "../ui/Select";
import Switch from "../ui/Switch";

const MODEL_9B_GB = "6.4";

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

export function supports9bModel(hw: { vramGb?: number; ramGb: number }): boolean {
  return hw.ramGb >= 16 || (hw.vramGb ?? 0) >= 8;
}

export function formatHardware(hw: { gpu?: string; vramGb?: number; ramGb: number }, model: VoiceModelTier): string {
  const parts: string[] = [];
  if (hw.gpu) {
    if (hw.vramGb != null) parts.push(`${hw.gpu} (${Math.round(hw.vramGb)} GB)`);
    else parts.push(hw.gpu);
  }
  parts.push(`${hw.ramGb.toFixed(1)} GB RAM`);
  const modelName = t(
    model === "light"
      ? "voice.model_light_short"
      : model === "9b"
        ? "m27.model_9b_short"
        : "voice.model_standard_short",
  );
  return t("voice.hw_summary", { details: parts.join(", "), model: modelName });
}

export interface VoiceSectionProps {
  initialStatus?: VoiceStatus;
  initialModelTier?: VoiceModelTier;
  initialNotices?: boolean;
  initialContext?: boolean;
  initialDuckMusic?: boolean;
  initialIslandRest?: "bar" | "lyrics";
  initialIslandBarHeight?: number;
  initialIslandBarPosition?: "left" | "center" | "right";
  initialCheckSteps?: VoiceCheckStep[];
  previewGuides?: SavedGuide[];
  previewModes?: WorkMode[];
  openAdvanced?: boolean;
}

/** Settings window: More → Voice (DATA_MODEL.md 12.7 + M27/M28 UI). */
export default function VoiceSection({
  initialStatus,
  initialModelTier,
  initialNotices = true,
  initialDuckMusic = true,
  initialIslandRest = "bar",
  initialIslandBarHeight = 10,
  initialIslandBarPosition = "center",
  initialCheckSteps,
  previewGuides,
  previewModes,
  openAdvanced,
}: VoiceSectionProps = {}) {
  const [status, setStatus] = useState<VoiceStatus>(initialStatus ?? DEFAULT_STATUS);
  const [modelTier, setModelTier] = useState<VoiceModelTier>(initialModelTier ?? (initialStatus?.model ?? "standard"));
  const [noticesEnabled, setNoticesEnabled] = useState<boolean>(initialNotices);
  const [duckMusic, setDuckMusic] = useState<boolean>(initialDuckMusic);
  const [island, setIsland] = useState<IslandSettings>({ restingMode: initialIslandRest, barHeight: initialIslandBarHeight, barPosition: initialIslandBarPosition });
  const [confirm9bOpen, setConfirm9bOpen] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (initialStatus) return;
    let active = true;
    call("voice_notices_get").then(
      (on) => active && setNoticesEnabled(on),
      (e) => active && setError(String(e)),
    );
    loadVoiceExtras(call).then(
      (extras) => {
        if (!active) return;
        setDuckMusic(extras.duckMusic);
        setIsland(extras.island);
      },
      (e) => active && setError(String(e)),
    );
    call("voice_status").then(
      (s) => {
        if (!active) return;
        setStatus(s);
        setModelTier((cur) => (cur === "9b" ? cur : s.model));
      },
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
  }, [initialStatus]);

  async function run(cmd: "voice_enable" | "voice_cancel" | "voice_remove" | "voice_redownload") {
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

  /** A change is shown at once and put back when the PC refuses it. */
  function changeDuck(enabled: boolean) {
    const before = duckMusic;
    setError(null);
    setDuckMusic(enabled);
    saveDuckMusic(call, enabled).then(setDuckMusic, (e) => {
      setDuckMusic(before);
      setError(String(e));
    });
  }

  function changeIsland(change: Partial<IslandSettings>) {
    const before = island;
    setError(null);
    setIsland({ ...island, ...change });
    saveIsland(call, change).then(setIsland, (e) => {
      setIsland(before);
      setError(String(e));
    });
  }

  function changeNotices(enabled: boolean) {
    const before = noticesEnabled;
    setError(null);
    setNoticesEnabled(enabled);
    call("voice_notices_set", { enabled }).then(setNoticesEnabled, (e) => {
      setNoticesEnabled(before);
      setError(String(e));
    });
  }

  return (
    <div className="page-container">
      <PageHeader icon="microphone" title={t("more.voice")} />
      {error && <Banner kind="error">{error}</Banner>}

      <SectionHeader id="voice-status" title={t("voice.title")} />
      {status.state === "off" && (
        <Group>
          <div style={{ padding: "14px 0" }}>
            <p>{t("voice.desc")}</p>
            <p className="muted">{formatHardware(status.hardware, status.model)}</p>
            <div style={{ marginTop: 12 }}>
              <Button variant="primary" disabled={busy} onClick={() => void run("voice_enable")}>
                {t("voice.turn_on", { size: status.downloadGb || (status.model === "light" ? "1.8" : "3.2") })}
              </Button>
              <p className="muted" style={{ fontSize: 12, marginTop: 8, marginBottom: 0 }}>
                {t("voice.privacy")}
              </p>
            </div>
          </div>
        </Group>
      )}

      {status.state === "downloading" && (
        <Group>
          <div style={{ padding: "14px 0" }}>
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
            <div style={{ marginTop: 12 }}>
              <Button disabled={busy} onClick={() => void run("voice_cancel")}>
                {t("common.cancel")}
              </Button>
            </div>
          </div>
        </Group>
      )}

      {status.state === "loading" && (
        <Group>
          <div style={{ padding: "14px 0" }}>
            <p className="muted">{t("voice.loading")}</p>
          </div>
        </Group>
      )}

      {status.state === "error" && (() => {
        const found = status.reason ? voiceErrorCopy(status.reason, status.exitCode) : null;
        // The light model is on already: there is nothing to switch to, only another try.
        const copy = found && found.action === "switch_light" && status.model === "light"
          ? { ...found, action: "retry" as const, buttonLabel: t("m28.err_btn_retry") }
          : found;
        return (
          <Group>
            <div style={{ padding: "14px 0" }}>
              <Banner kind="error">
                {copy ? (
                  <>
                    <strong>{copy.reason}</strong>
                    <div style={{ marginTop: 4 }}>{copy.hint}</div>
                  </>
                ) : (
                  status.error ?? ""
                )}
              </Banner>
              <div style={{ marginTop: 12, display: "flex", gap: 8 }}>
                {copy ? (
                  <Button
                    variant="primary"
                    disabled={busy}
                    onClick={() => {
                      if (copy.action === "switch_light") {
                        setModelTier("light");
                        void setConfig({ model: "light" });
                      } else if (copy.action === "check_mic") {
                        document.getElementById("voice-check")?.scrollIntoView({ behavior: "smooth" });
                      } else if (copy.action === "redownload") {
                        void run("voice_redownload");
                      } else {
                        void run("voice_enable");
                      }
                    }}
                  >
                    {copy.buttonLabel}
                  </Button>
                ) : (
                  <Button variant="primary" disabled={busy} onClick={() => void run("voice_enable")}>
                    {t("voice.turn_on", { size: status.downloadGb || "3.2" })}
                  </Button>
                )}
              </div>
            </div>
          </Group>
        );
      })()}

      {/* T28.1: the 4B model on a PC with little memory: offered, never switched by itself. */}
      {status.suggestLight && status.state !== "downloading" && !(status.state === "error" && (status.reason === "vram" || status.reason === "ram")) && (
        <Group>
          <Row id="section-voice-light" title={t("voice.light_better")}>
            <Button
              disabled={busy}
              onClick={() => {
                setModelTier("light");
                void setConfig({ model: "light" });
              }}
            >
              {t("m28.err_btn_switch_light")}
            </Button>
          </Row>
        </Group>
      )}

      {status.state === "ready" && (
        <>
          <Group>
            <Row id="section-voice-hotkey" title={t("voice.hotkey")} description={t("voice.ready", { hotkey: formatChord(status.hotkey) })}>
              <ChordField compact keys={status.hotkey}onChange={(keys) => void setConfig({ hotkey: keys })} />
            </Row>

            <Row id="section-voice-mic" title={t("voice.mic")}>
              <Select
                aria-label={t("voice.mic")}
                value={status.mic ?? ""}
                options={[
                  { value: "", label: t("voice.mic_default") },
                  ...status.mics.map((m) => ({ value: m, label: m })),
                ]}
                onChange={(mic) => void setConfig({ mic })}
              />
            </Row>

            <Row title={t("voice.keep_loaded")} description={t("voice.keep_loaded_hint")}>
              <Switch checked={status.keepLoaded} onChange={(val) => void setConfig({ keepLoaded: val })} />
            </Row>

            <Row title={t("voice.phone_button")}>
              <Switch checked={status.phoneButton} onChange={(val) => void setConfig({ phoneButton: val })} />
            </Row>

            <Row title={t("m27.notices_title")} description={t("m27.notices_desc")}>
              <Switch
                checked={noticesEnabled}
                onChange={changeNotices}
              />
            </Row>

            <Row id="section-voice-duck" title={t("m29.voice_duck")} description={t("m29.voice_duck_hint")}>
              <Switch checked={duckMusic} onChange={changeDuck} />
            </Row>

            <Row id="section-voice-speak" title={t("m29.voice_speak")} description={t("m29.voice_speak_hint")}>
              <Switch checked={status.speak} onChange={(val) => void setConfig({ speak: val })} />
            </Row>
          </Group>

          <SectionHeader title={t("m29.island_title")} />
          <Group>
            <Row id="section-island-rest" title={t("m29.island_rest")}>
              <Select
                aria-label={t("m29.island_rest")}
                value={island.restingMode}
                options={[
                  { value: "bar", label: t("m29.island_rest_bar") },
                  { value: "lyrics", label: t("m29.island_rest_lyrics") },
                ]}
                onChange={(val) => {
                  changeIsland({ restingMode: val as IslandSettings["restingMode"] });
                }}
              />
            </Row>

            <Row id="section-island-bar-height" title={t("m29.island_bar_height")}>
              <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
                <input
                  type="range"
                  aria-label={t("m29.island_bar_height")}
                  min={6}
                  max={16}
                  step={1}
                  value={island.barHeight}
                  onChange={(e) => {
                    changeIsland({ barHeight: Number(e.target.value) });
                  }}
                />
                <span style={{ fontSize: "var(--font-sm)", minWidth: 42, textAlign: "right", color: "var(--text-muted)" }}>
                  {island.barHeight} px
                </span>
              </div>
            </Row>

            <Row id="section-island-bar-pos" title={t("m29.island_bar_pos")}>
              <Select
                aria-label={t("m29.island_bar_pos")}
                value={island.barPosition}
                options={[
                  { value: "left", label: t("m29.island_pos_left") },
                  { value: "center", label: t("m29.island_pos_center") },
                  { value: "right", label: t("m29.island_pos_right") },
                ]}
                onChange={(val) => {
                  changeIsland({ barPosition: val as IslandSettings["barPosition"] });
                }}
              />
            </Row>
          </Group>

          <Disclosure title={t("voice.advanced")} defaultOpen={openAdvanced} style={{ marginTop: 16 }}>
            <Group>
              <Row
                title={t("voice.model")}
                description={
                  supports9bModel(status.hardware)
                    ? t("voice.models_info", {
                        llm: modelTier === "light" ? "Qwen3.5 2B" : modelTier === "9b" ? "Qwen3.5 9B" : "Qwen3.5 4B",
                      })
                    : t("m27.model_9b_req")
                }
              >
                <Select
                  aria-label={t("voice.model")}
                  value={modelTier}
                  options={[
                    { value: "standard", label: t("voice.model_standard") },
                    { value: "light", label: t("voice.model_light") },
                    ...(supports9bModel(status.hardware) ? [{ value: "9b", label: t("m27.model_9b") }] : []),
                  ]}
                  onChange={(val) => {
                    const tier = val as VoiceModelTier;
                    if (tier === "9b") {
                      setConfirm9bOpen(true);
                      return;
                    }
                    setModelTier(tier);
                    void setConfig({ model: tier });
                  }}
                />
              </Row>
              <Row title={t("voice.remove")}>
                <Button variant="danger" disabled={busy} onClick={() => void run("voice_remove")}>
                  {t("voice.remove")}
                </Button>
              </Row>
            </Group>
          </Disclosure>
        </>
      )}

      {confirm9bOpen && (
        <Modal
          title={t("m27.model_9b_confirm_title")}
          onClose={() => setConfirm9bOpen(false)}
          footer={
            <>
              <Button onClick={() => setConfirm9bOpen(false)}>{t("common.cancel")}</Button>
              <Button
                variant="primary"
                onClick={() => {
                  setModelTier("9b");
                  setConfirm9bOpen(false);
                  call("voice_set_model_tier", { model: "9b" }).then(setModelTier, (e) => {
                    setModelTier(status.model);
                    setError(String(e));
                  });
                }}
              >
                {t("m27.model_9b_confirm_ok", { size: MODEL_9B_GB })}
              </Button>
            </>
          }
        >
          <p>{t("m27.model_9b_confirm_body", { size: MODEL_9B_GB })}</p>
        </Modal>
      )}

      {status.state !== "off" && status.state !== "downloading" && <VoiceCheckPanel initialSteps={initialCheckSteps} />}
      <DictationBlock />
      <MemoryBlock />
      <GuidesBlock initialGuides={previewGuides} />
      <ModesBlock initialModes={previewModes} />
      <div id="voice-help" style={{ marginTop: 24 }}>
        <VoiceCommands />
      </div>
    </div>
  );
}
