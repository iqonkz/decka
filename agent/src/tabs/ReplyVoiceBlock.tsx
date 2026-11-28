import { useEffect, useRef, useState } from "react";
import { t } from "@decka/shared";
import Button from "../ui/Button";
import Segmented from "../ui/Segmented";
import Select from "../ui/Select";
import { Group, Row, SectionHeader } from "../ui/SettingsGroup";
import {
  clampReplySpeed,
  DEFAULT_REPLY_SPEED,
  findPiperVoice,
  MAX_REPLY_SPEED,
  MIN_REPLY_SPEED,
  PIPER_VOICES,
  REPLY_SPEED_STEP,
  type ReplyVoiceConfig,
  type ReplyVoiceEngine,
} from "./voiceM33Types";

const REPLY_VOICE_KEY = "decka.replyvoice.v1";

function defaultReplyVoice(): ReplyVoiceConfig {
  return {
    engine: "piper",
    voiceId: PIPER_VOICES[0]!.id,
    speed: DEFAULT_REPLY_SPEED,
    downloadedIds: [],
  };
}

function loadStoredReplyVoice(): ReplyVoiceConfig {
  if (typeof window === "undefined") return defaultReplyVoice();
  try {
    const raw = window.localStorage.getItem(REPLY_VOICE_KEY);
    if (!raw) return defaultReplyVoice();
    const parsed = JSON.parse(raw) as Partial<ReplyVoiceConfig>;
    return {
      engine: parsed.engine === "windows" ? "windows" : "piper",
      voiceId: findPiperVoice(parsed.voiceId ?? "").id,
      speed: clampReplySpeed(parsed.speed ?? DEFAULT_REPLY_SPEED),
      downloadedIds: Array.isArray(parsed.downloadedIds)
        ? parsed.downloadedIds.filter((id): id is string => typeof id === "string")
        : [],
    };
  } catch (err) {
    console.warn("reply voice storage read failed:", err);
    return defaultReplyVoice();
  }
}

function saveStoredReplyVoice(cfg: ReplyVoiceConfig): void {
  if (typeof window === "undefined") return;
  try {
    window.localStorage.setItem(REPLY_VOICE_KEY, JSON.stringify(cfg));
  } catch (err) {
    console.warn("reply voice storage write failed:", err);
  }
}

export interface ReplyVoiceBlockProps {
  initialConfig?: Partial<ReplyVoiceConfig>;
  initialDownloadPct?: number | null;
}

/**
 * M33 T33.1b: Reply voice engine selector (Piper / Windows), per-language voice picker with size
 * and Download/Listen/Remove actions, and speech speed slider (0.8-1.3, default 1.05).
 */
export default function ReplyVoiceBlock({
  initialConfig,
  initialDownloadPct = null,
}: ReplyVoiceBlockProps = {}) {
  const [cfg, setCfg] = useState<ReplyVoiceConfig>(() => {
    const base = loadStoredReplyVoice();
    if (!initialConfig) return base;
    return {
      engine: initialConfig.engine ?? base.engine,
      voiceId: findPiperVoice(initialConfig.voiceId ?? base.voiceId).id,
      speed: clampReplySpeed(initialConfig.speed ?? base.speed),
      downloadedIds: initialConfig.downloadedIds ?? base.downloadedIds,
    };
  });
  const [downloadPct, setDownloadPct] = useState<number | null>(initialDownloadPct);
  const [playingSample, setPlayingSample] = useState(false);
  const [errorText, setErrorText] = useState<string | null>(null);
  const timerRef = useRef<number | null>(null);
  const sampleTimerRef = useRef<number | null>(null);

  useEffect(() => {
    return () => {
      if (timerRef.current !== null) window.clearInterval(timerRef.current);
      if (sampleTimerRef.current !== null) window.clearTimeout(sampleTimerRef.current);
    };
  }, []);

  const update = (patch: Partial<ReplyVoiceConfig>) => {
    setCfg((prev) => {
      const next: ReplyVoiceConfig = { ...prev, ...patch };
      saveStoredReplyVoice(next);
      return next;
    });
  };

  const selectedVoice = findPiperVoice(cfg.voiceId);
  const isDownloaded = cfg.downloadedIds.includes(selectedVoice.id);
  const isDownloading = downloadPct !== null;

  const startDownload = () => {
    if (isDownloading) return;
    setErrorText(null);
    if (timerRef.current !== null) window.clearInterval(timerRef.current);
    setDownloadPct(15);
    let pct = 15;
    timerRef.current = window.setInterval(() => {
      pct += 25;
      if (pct >= 100) {
        if (timerRef.current !== null) {
          window.clearInterval(timerRef.current);
          timerRef.current = null;
        }
        setDownloadPct(null);
        setCfg((prev) => {
          const downloadedIds = prev.downloadedIds.includes(selectedVoice.id)
            ? prev.downloadedIds
            : [...prev.downloadedIds, selectedVoice.id];
          const next = { ...prev, downloadedIds };
          saveStoredReplyVoice(next);
          return next;
        });
        return;
      }
      setDownloadPct(pct);
    }, 180);
  };

  const removeVoice = () => {
    if (timerRef.current !== null) {
      window.clearInterval(timerRef.current);
      timerRef.current = null;
    }
    setDownloadPct(null);
    setPlayingSample(false);
    setErrorText(null);
    update({ downloadedIds: cfg.downloadedIds.filter((id) => id !== selectedVoice.id) });
  };

  const listenSample = () => {
    setErrorText(null);
    setPlayingSample(true);
    if (sampleTimerRef.current !== null) window.clearTimeout(sampleTimerRef.current);
    try {
      if (typeof window !== "undefined" && "speechSynthesis" in window) {
        window.speechSynthesis.cancel();
        const utter = new SpeechSynthesisUtterance(selectedVoice.name);
        utter.lang = selectedVoice.lang;
        utter.rate = cfg.speed;
        utter.onend = () => setPlayingSample(false);
        utter.onerror = () => setPlayingSample(false);
        window.speechSynthesis.speak(utter);
      }
    } catch (err) {
      setErrorText(err instanceof Error ? err.message : String(err));
    }
    sampleTimerRef.current = window.setTimeout(() => setPlayingSample(false), 1200);
  };

  return (
    <div id="voice-reply-voice" style={{ marginTop: 16 }}>
      <SectionHeader title={t("m33.reply_voice_title")} />
      <Group>
        <Row
          id="section-reply-engine"
          title={t("m33.engine_label")}
          description={cfg.engine === "windows" ? t("m33.engine_windows_hint") : t("m33.voice_lang_note")}
          icon="waveform"
        >
          <Segmented
            options={[
              { value: "piper", label: t("m33.engine_piper") },
              { value: "windows", label: t("m33.engine_windows") },
            ]}
            value={cfg.engine}
            onChange={(val) => update({ engine: val as ReplyVoiceEngine })}
          />
        </Row>

        {cfg.engine === "piper" && (
          <Row
            id="section-reply-piper-voice"
            title={t("m33.voice_label")}
            description={
              <span style={{ display: "inline-flex", alignItems: "center", gap: 8, flexWrap: "wrap" }}>
                <span className={`voice-check-status ${isDownloaded ? "ok" : "wait"}`}>
                  {isDownloading
                    ? t("m33.downloading", { pct: downloadPct ?? 0 })
                    : isDownloaded
                      ? t("m33.downloaded_badge")
                      : t("m33.not_downloaded_badge")}
                </span>
                <span>{t("m33.size_mb", { mb: selectedVoice.sizeMb })}</span>
                {errorText && <span style={{ color: "var(--danger, #dc2626)" }}>{errorText}</span>}
              </span>
            }
            icon="speaker-high"
          >
            <div style={{ display: "flex", alignItems: "center", gap: 8, flexWrap: "wrap", justifyContent: "flex-end" }}>
              <Select
                aria-label={t("m33.voice_label")}
                value={selectedVoice.id}
                options={PIPER_VOICES.map((v) => ({
                  value: v.id,
                  label: `${v.name} · ${t("m33.size_mb", { mb: v.sizeMb })}`,
                }))}
                onChange={(voiceId) => {
                  setDownloadPct(null);
                  setPlayingSample(false);
                  update({ voiceId });
                }}
              />
              {!isDownloaded && !isDownloading && (
                <Button variant="primary" onClick={startDownload}>
                  {t("m33.download")}
                </Button>
              )}
              {isDownloading && (
                <Button disabled onClick={() => {}}>
                  {t("m33.downloading", { pct: downloadPct ?? 0 })}
                </Button>
              )}
              {isDownloaded && (
                <>
                  <Button onClick={listenSample}>
                    {playingSample ? t("m33.listening_sample") : t("m33.listen")}
                  </Button>
                  <Button variant="danger" onClick={removeVoice}>
                    {t("m33.remove")}
                  </Button>
                </>
              )}
            </div>
          </Row>
        )}

        <Row
          id="section-reply-speed"
          title={t("m33.speed_label")}
          description={
            cfg.speed !== DEFAULT_REPLY_SPEED ? (
              <button
                type="button"
                onClick={() => update({ speed: DEFAULT_REPLY_SPEED })}
                style={{
                  background: "none",
                  border: 0,
                  padding: 0,
                  color: "var(--accent)",
                  cursor: "pointer",
                  font: "inherit",
                  fontSize: "var(--font-xs)",
                }}
              >
                {t("m33.speed_reset")}
              </button>
            ) : undefined
          }
          icon="gauge"
        >
          <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
            <input
              type="range"
              aria-label={t("m33.speed_label")}
              min={MIN_REPLY_SPEED}
              max={MAX_REPLY_SPEED}
              step={REPLY_SPEED_STEP}
              value={cfg.speed}
              onChange={(e) => update({ speed: clampReplySpeed(Number(e.target.value)) })}
            />
            <span
              style={{
                fontSize: "var(--font-sm)",
                fontVariantNumeric: "tabular-nums",
                minWidth: 44,
                textAlign: "right",
                color: "var(--text-muted)",
              }}
            >
              {cfg.speed.toFixed(2)}×
            </span>
          </div>
        </Row>
      </Group>
    </div>
  );
}
