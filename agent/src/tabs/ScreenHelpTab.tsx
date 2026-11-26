import { useEffect, useState } from "react";
import { t, type VisionPack } from "@decka/shared";
import { call } from "../ipc";
import Banner from "../ui/Banner";
import Button from "../ui/Button";
import { Group, PageHeader, Row, SectionHeader } from "../ui/SettingsGroup";
import Switch from "../ui/Switch";

export interface ScreenHelpTabProps {
  initialEnabled?: boolean;
  initialPackReady?: boolean;
  initialPackSize?: string;
}

/** `0.67 GB`: the size of the package on the disk. */
export const packSize = (bytes: number): string => `${(bytes / 1e9).toFixed(2)} GB`;

const EXAMPLES = ["m29.screen_example_1", "m29.screen_example_2", "m29.screen_example_3", "m29.screen_example_4"] as const;

/** T29.13b: Dedicated sidebar page for Screen assist (moved out of VoiceSection). */
export default function ScreenHelpTab({ initialEnabled = false, initialPackReady = false, initialPackSize = "" }: ScreenHelpTabProps = {}) {
  const [enabled, setEnabled] = useState(initialEnabled);
  const [pack, setPack] = useState<VisionPack | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const ready = pack ? pack.installed : initialPackReady;
  const size = pack ? packSize(pack.size) : initialPackSize;
  const percent = pack?.percent ?? null;

  useEffect(() => {
    let active = true;
    const failed = (e: unknown) => active && setError(String(e));
    call("voice_context_get").then((on) => active && setEnabled(on), failed);
    call("voice_context_pack_status").then((p) => active && setPack(p), failed);
    return () => {
      active = false;
    };
  }, []);

  // While a download runs (started here or by a spoken "yes"), the percent is asked once a second.
  useEffect(() => {
    if (!busy && percent === null) return;
    const timer = window.setInterval(() => {
      call("voice_context_pack_status").then(setPack, (e) => setError(String(e)));
    }, 1000);
    return () => window.clearInterval(timer);
  }, [busy, percent === null]);

  function toggleEnabled(next: boolean) {
    setError(null);
    setEnabled(next);
    call("voice_context_set", { enabled: next }).then(setEnabled, (e) => {
      setEnabled(!next);
      setError(String(e));
    });
  }

  async function handlePack(command: "voice_context_pack_download" | "voice_context_pack_remove") {
    setBusy(true);
    setError(null);
    try {
      setPack(await call(command));
    } catch (e) {
      setError(String(e));
      call("voice_context_pack_status").then(setPack, () => setPack(null));
    } finally {
      setBusy(false);
    }
  }

  const state = percent !== null ? t("m29.screen_pack_downloading", { pct: percent }) : ready ? t("m29.screen_pack_ready") : t("m29.screen_pack_missing");

  return (
    <div className="page-container">
      <PageHeader icon="monitor" title={t("m28.screen_help_title")} subtitle={t("m28.screen_help_desc")} />
      {error && <Banner kind="error">{error}</Banner>}

      <Group>
        <Row id="section-screen-help-toggle" title={t("m28.screen_help_title")} description={t("m28.screen_help_desc")}>
          <Switch checked={enabled} onChange={toggleEnabled} />
        </Row>

        <Row
          id="section-screen-help-pack"
          title={t("m29.screen_pack_title")}
          description={size ? `${state} · ${t("m29.screen_pack_size", { size })}` : state}
        >
          {ready ? (
            <Button variant="danger" disabled={busy} onClick={() => void handlePack("voice_context_pack_remove")}>
              {t("m29.screen_pack_remove")}
            </Button>
          ) : (
            <Button variant="primary" disabled={busy || percent !== null} onClick={() => void handlePack("voice_context_pack_download")}>
              {t("m29.screen_pack_download")}
            </Button>
          )}
        </Row>
      </Group>

      <SectionHeader title={t("m29.screen_what_reads")} />
      <Group>
        <div style={{ padding: "12px 16px", display: "flex", flexDirection: "column", gap: 6, fontSize: "var(--font-sm)", color: "var(--text-muted)" }}>
          <div>• {t("m29.screen_rule_1")}</div>
          <div>• {t("m29.screen_rule_2")}</div>
          <div>• {t("m29.screen_rule_3")}</div>
        </div>
      </Group>

      <SectionHeader title={t("m29.screen_examples_title")} />
      <Group>
        <div style={{ padding: "12px 16px", display: "flex", flexWrap: "wrap", gap: 8 }}>
          {EXAMPLES.map((key) => (
            <span
              key={key}
              style={{
                fontSize: "var(--font-sm)",
                padding: "6px 10px",
                borderRadius: "var(--r-control)",
                background: "var(--bg-subtle, rgba(128,128,128,0.08))",
                border: "1px solid var(--border)",
              }}
            >
              «{t(key)}»
            </span>
          ))}
        </div>
      </Group>
    </div>
  );
}
