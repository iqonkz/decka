import { useEffect, useState } from "react";
import { t, type DictationSettings, type DictationTrigger } from "@decka/shared";
import { call } from "../ipc";
import Banner from "../ui/Banner";
import ChordField from "../editor/ChordField";
import { formatChord } from "../editor/addButton";
import { Group, Row, SectionHeader } from "../ui/SettingsGroup";
import Segmented from "../ui/Segmented";

const DEFAULT_CHORD = ["ctrl", "alt", "d"];

/** The dictionary as the field shows it: one phrase per line. */
export const toLines = (items: string[]): string => items.join("\n");
/** The lines of the field as the list the PC stores (it tidies it again: trimmed, no repeats, at most 500 of 60 characters). */
export const fromLines = (text: string): string[] => text.split(/\r?\n/).map((l) => l.trim()).filter((l) => l !== "");

/** `More -> Voice -> Dictation` (DATA_MODEL.md 13.7): what starts a dictation, the mode and the dictionary. */
export default function DictationBlock() {
  const [settings, setSettings] = useState<DictationSettings | null>(null);
  const [dictionary, setDictionary] = useState("");
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    call("voice_dictation_get").then(
      (s) => {
        setSettings(s);
        setDictionary(toLines(s.dictionary));
      },
      (e) => setError(String(e)),
    );
  }, []);
  if (!settings) return error ? <Banner kind="error">{error}</Banner> : null;

  async function save(next: DictationSettings) {
    setError(null);
    try {
      const saved = await call("voice_dictation_set", next);
      setSettings(saved);
    } catch (e) {
      setError(String(e));
    }
  }
  const kind = settings.trigger.kind;
  const setTrigger = (k: DictationTrigger["kind"]) => {
    const trigger: DictationTrigger = k === "chord" ? { kind: "chord", keys: settings.trigger.kind === "chord" ? settings.trigger.keys : DEFAULT_CHORD } : { kind: k };
    void save({ ...settings, trigger });
  };
  const lines = fromLines(dictionary);
  return (
    <div id="voice-dictation" style={{ marginTop: 24 }}>
      <SectionHeader id="section-voice-dictation" title={t("dict.title")} />
      {error && <Banner kind="error">{error}</Banner>}
      <Group>
        <Row title={t("dict.trigger")}>
          <select className="settings-select" value={kind} onChange={(e) => setTrigger(e.target.value as DictationTrigger["kind"])}>
            <option value="doubleCtrl">{t("dict.trigger_double")}</option>
            <option value="chord">{t("dict.trigger_chord")}</option>
            <option value="off">{t("dict.trigger_off")}</option>
          </select>
        </Row>
        {settings.trigger.kind === "chord" && (
          <Row title={t("dict.trigger_chord")} description={formatChord(settings.trigger.keys)}>
            <ChordField compact keys={settings.trigger.keys} onChange={(keys) => void save({ ...settings, trigger: { kind: "chord", keys } })} />
          </Row>
        )}
        <Row title={t("dict.mode")} description={t(settings.mode === "clean" ? "dict.mode_clean_hint" : "dict.mode_raw_hint")}>
          <Segmented
            options={[
              { value: "clean", label: t("dict.mode_clean") },
              { value: "raw", label: t("dict.mode_raw") },
            ]}
            value={settings.mode}
            onChange={(val) => void save({ ...settings, mode: val })}
          />
        </Row>
        <div style={{ padding: "14px 0" }}>
          <div style={{ marginBottom: 6 }}>
            <span className="settings-row-title">{t("dict.dictionary")}</span>
            <span className="muted" style={{ display: "block", fontSize: 13, marginTop: 2 }}>
              {t("dict.dictionary_hint", { n: lines.length })}
            </span>
          </div>
          <textarea rows={5} spellCheck={false} value={dictionary} onChange={(e) => setDictionary(e.target.value)} onBlur={() => lines.join("\n") !== settings.dictionary.join("\n") && void save({ ...settings, dictionary: lines })} />
        </div>
      </Group>
    </div>
  );
}
