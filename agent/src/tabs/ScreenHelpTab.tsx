import { useEffect, useState } from "react";
import { locale, t } from "@decka/shared";
import { invoke } from "@tauri-apps/api/core";
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

/** T29.13b: Dedicated sidebar page for Screen assist (moved out of VoiceSection). */
export default function ScreenHelpTab({
  initialEnabled = false,
  initialPackReady = false,
  initialPackSize = "1.8 GB",
}: ScreenHelpTabProps = {}) {
  const [enabled, setEnabled] = useState(initialEnabled);
  const [packReady, setPackReady] = useState(initialPackReady);
  const [packSize] = useState(initialPackSize);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let active = true;
    call("voice_context_get").then(
      (on) => active && setEnabled(on),
      () => {},
    );
    return () => {
      active = false;
    };
  }, []);

  async function toggleEnabled(next: boolean) {
    setError(null);
    setEnabled(next);
    try {
      const actual = await call("voice_context_set", { enabled: next });
      setEnabled(actual);
    } catch (e) {
      setError(String(e));
    }
  }

  async function handlePack(action: "download" | "remove") {
    setBusy(true);
    setError(null);
    try {
      await invoke(action === "download" ? "voice_context_pack_download" : "voice_context_pack_remove").catch(() => {});
      setPackReady(action === "download");
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }

  const isRu = locale() === "ru";
  const examples = isRu
    ? [
        "Что сейчас на экране?",
        "Где кнопка «Сохранить»?",
        "Нажми на кнопку «Отправить»",
        "Покажи по шагам, как экспортировать файл",
      ]
    : [
        "What is on my screen?",
        "Where is the Save button?",
        "Click the Send button",
        "Guide me step by step to export a file",
      ];

  return (
    <div className="page-container">
      <PageHeader icon="monitor" title={t("m28.screen_help_title")} subtitle={t("m28.screen_help_desc")} />
      {error && <Banner kind="error">{error}</Banner>}

      <Group>
        <Row id="section-screen-help-toggle" title={t("m28.screen_help_title")} description={t("m28.screen_help_desc")}>
          <Switch checked={enabled} onChange={(val) => void toggleEnabled(val)} />
        </Row>

        <Row
          id="section-screen-help-pack"
          title={t("m29.screen_pack_title")}
          description={`${packReady ? t("m29.screen_pack_ready") : t("m29.screen_pack_missing")} · ${t("m29.screen_pack_size", { size: packSize })}`}
        >
          {packReady ? (
            <Button variant="danger" disabled={busy} onClick={() => void handlePack("remove")}>
              {t("m29.screen_pack_remove")}
            </Button>
          ) : (
            <Button variant="primary" disabled={busy} onClick={() => void handlePack("download")}>
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
          {examples.map((ex) => (
            <span
              key={ex}
              style={{
                fontSize: "var(--font-sm)",
                padding: "6px 10px",
                borderRadius: "var(--r-control)",
                background: "var(--bg-subtle, rgba(128,128,128,0.08))",
                border: "1px solid var(--border)",
              }}
            >
              «{ex}»
            </span>
          ))}
        </div>
      </Group>
    </div>
  );
}
