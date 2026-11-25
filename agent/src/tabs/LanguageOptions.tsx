import { LANGUAGE_NAMES, languageOrder, resolveLanguage, t } from "@decka/shared";
import type { SelectOption } from "../ui/Select";

/** The options of a language list (DESIGN.md 6): `Auto (system)`, then the language of the system and English, then the rest by their own name. */
export function languageSelectOptions(): SelectOption[] {
  return [
    { value: "auto", label: t("settings.language_auto") },
    ...languageOrder(resolveLanguage("auto", navigator.language)).map((code) => ({
      value: code,
      label: LANGUAGE_NAMES[code],
    })),
  ];
}

export default function LanguageOptions() {
  return null;
}
