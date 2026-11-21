import { locale, t } from "@decka/shared";
import ar from "../../../shared/voice/ar.json";
import de from "../../../shared/voice/de.json";
import en from "../../../shared/voice/en.json";
import es from "../../../shared/voice/es.json";
import fr from "../../../shared/voice/fr.json";
import hi from "../../../shared/voice/hi.json";
import ja from "../../../shared/voice/ja.json";
import kk from "../../../shared/voice/kk.json";
import pt from "../../../shared/voice/pt.json";
import ru from "../../../shared/voice/ru.json";
import zh from "../../../shared/voice/zh.json";

interface VoiceTable {
  lang: string;
  yes: string[];
  no: string[];
  intents: Array<{
    tool: string;
    args?: Record<string, unknown>;
    patterns: string[];
  }>;
}

const TABLES: Record<string, VoiceTable> = {
  ar,
  de,
  en,
  es,
  fr,
  hi,
  ja,
  kk,
  pt,
  ru,
  zh,
};

function formatPattern(pattern: string, lang: string): string {
  const calcNames: Record<string, string> = {
    ru: "Калькулятор",
    kk: "Калькулятор",
    zh: "计算器",
    ja: "電卓",
    hi: "कैलकुलेटर",
    ar: "الحاسبة",
    fr: "Calculatrice",
    es: "Calculadora",
    pt: "Calculadora",
    de: "Rechner",
    en: "Calculator",
  };
  const calc = calcNames[lang] ?? "Calculator";

  return pattern
    .replace("{app}", calc)
    .replace("{name}", "Streaming")
    .replace("{query}", "lofi")
    .replace("{n}", "10")
    .replace("{label}", "Mute")
    .replace("{domain}", "github.com")
    .replace("{text}", "Hello");
}

export function getExamplesForCategory(
  table: VoiceTable,
  tools: string[],
  limit = 3,
): string[] {
  const results: string[] = [];
  for (const intent of table.intents) {
    if (tools.includes(intent.tool)) {
      if (intent.patterns[0]) {
        results.push(formatPattern(intent.patterns[0], table.lang));
        if (results.length >= limit) break;
      }
    }
  }
  return results;
}

export default function VoiceHelp() {
  const currentLang = locale();
  const table = TABLES[currentLang] ?? TABLES.en ?? en;

  const categories = [
    {
      title: t("voice.cat_apps"),
      tools: ["open_app", "close_app", "switch_app"],
    },
    {
      title: t("voice.cat_media"),
      tools: ["volume", "media"],
    },
    {
      title: t("voice.cat_deck"),
      tools: ["press_button", "go_page"],
    },
    {
      title: t("voice.cat_system"),
      tools: ["timer", "web_search", "system"],
    },
  ];

  return (
    <div className="voice-help-section">
      <h3 style={{ margin: "20px 0 10px" }}>{t("voice.help_title")}</h3>
      <div className="voice-help-grid">
        {categories.map((cat) => {
          const examples = getExamplesForCategory(table, cat.tools, 3);
          return (
            <div key={cat.title} className="card voice-help-card">
              <h4>{cat.title}</h4>
              <ul className="voice-examples-list">
                {examples.map((ex, i) => (
                  <li key={i}>"{ex}"</li>
                ))}
              </ul>
            </div>
          );
        })}
      </div>
      <p className="muted voice-limit">{t("voice.help_limit")}</p>
    </div>
  );
}
