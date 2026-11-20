import { LANGUAGE_NAMES, languageOrder, resolveLanguage, t } from "@decka/shared";

/** The options of a language list (DESIGN.md 6): `Auto (system)`, then the language of the system and English, then the rest by their own name. */
export default function LanguageOptions() {
  return (
    <>
      <option value="auto">{t("settings.language_auto")}</option>
      {languageOrder(resolveLanguage("auto", navigator.language)).map((code) => (
        <option key={code} value={code}>
          {LANGUAGE_NAMES[code]}
        </option>
      ))}
    </>
  );
}
