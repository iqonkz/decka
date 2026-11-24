import { useEffect, useState } from "react";
import { t, type DictationSettings, type DictationTrigger } from "@decka/shared";
import { call } from "../ipc";
import Banner from "../ui/Banner";
import Field from "../ui/Field";
import ChordField from "../editor/ChordField";

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
    <section className="card" id="dictation">
      <h3>{t("dict.title")}</h3>
      {error && <Banner kind="error">{error}</Banner>}
      <div className="stack" style={{ gap: 12 }}>
        <Field label={t("dict.trigger")}>
          <select value={kind} onChange={(e) => setTrigger(e.target.value as DictationTrigger["kind"])}>
            <option value="doubleCtrl">{t("dict.trigger_double")}</option>
            <option value="chord">{t("dict.trigger_chord")}</option>
            <option value="off">{t("dict.trigger_off")}</option>
          </select>
        </Field>
        {settings.trigger.kind === "chord" && <ChordField keys={settings.trigger.keys} onChange={(keys) => void save({ ...settings, trigger: { kind: "chord", keys } })} />}
        <Field label={t("dict.mode")} hint={t(settings.mode === "clean" ? "dict.mode_clean_hint" : "dict.mode_raw_hint")}>
          <select value={settings.mode} onChange={(e) => void save({ ...settings, mode: e.target.value as "clean" | "raw" })}>
            <option value="clean">{t("dict.mode_clean")}</option>
            <option value="raw">{t("dict.mode_raw")}</option>
          </select>
        </Field>
        <Field label={t("dict.dictionary")} hint={t("dict.dictionary_hint", { n: lines.length })}>
          <textarea rows={5} spellCheck={false} value={dictionary} onChange={(e) => setDictionary(e.target.value)} onBlur={() => lines.join("\n") !== settings.dictionary.join("\n") && void save({ ...settings, dictionary: lines })} />
        </Field>
      </div>
    </section>
  );
}