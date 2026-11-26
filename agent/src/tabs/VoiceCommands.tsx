import { useMemo, useState } from "react";
import { locale, t, type TranslationKey } from "@decka/shared";
import { ChevronDown, ChevronRight, Search } from "lucide-react";
import { Group, SectionHeader } from "../ui/SettingsGroup";
import { VOICE_COMMAND_GROUPS, type CommandBadge } from "./voiceCommandsData";

const BADGE_KEYS: Record<CommandBadge, TranslationKey> = {
  no_ai: "m29.cmd_badge_no_ai",
  ai: "m29.cmd_badge_ai",
  screen: "m29.cmd_badge_screen",
  confirm: "m29.cmd_badge_confirm",
};

export interface VoiceCommandsProps {
  initialQuery?: string;
}

/** T29.12b: Full searchable voice command reference with 11 collapsible groups (collapsed by default). */
export default function VoiceCommands({ initialQuery = "" }: VoiceCommandsProps = {}) {
  const [query, setQuery] = useState(initialQuery);
  const [openIds, setOpenIds] = useState<Record<string, boolean>>({});
  const isRu = locale() === "ru";

  const q = query.trim().toLowerCase();

  const filtered = useMemo(() => {
    return VOICE_COMMAND_GROUPS.map((g) => {
      const title = t(g.titleKey);
      const desc = t(g.descKey);
      const groupMatch = q === "" || title.toLowerCase().includes(q) || desc.toLowerCase().includes(q);
      const examples = g.examples.filter((ex) => {
        if (groupMatch || q === "") return true;
        const phrase = isRu ? ex.ru : ex.en;
        return phrase.toLowerCase().includes(q) || ex.tool.toLowerCase().includes(q);
      });
      return { ...g, title, desc, examples };
    }).filter((g) => g.examples.length > 0);
  }, [q, isRu]);

  function toggle(id: string) {
    setOpenIds((cur) => ({ ...cur, [id]: !cur[id] }));
  }

  return (
    <div id="voice-commands">
      <SectionHeader title={t("m29.cmd_title")} />
      <div className="integrations-search">
        <Search size={16} aria-hidden="true" style={{ color: "var(--muted)", flex: "none" }} />
        <input
          type="search"
          className="integrations-search-input"
          aria-label={t("m29.cmd_search")}
          placeholder={t("m29.cmd_search")}
          value={query}
          onChange={(e) => setQuery(e.target.value)}
        />
      </div>

      {filtered.length === 0 ? (
        <Group>
          <div style={{ padding: "14px 16px", fontSize: "var(--font-sm)", color: "var(--text-muted)" }}>
            {t("m29.cmd_empty")}
          </div>
        </Group>
      ) : (
        <Group>
          {filtered.map((g, idx) => {
            const isOpen = q !== "" || Boolean(openIds[g.id]);
            return (
              <div
                key={g.id}
                style={{
                  borderTop: idx > 0 ? "1px solid var(--border)" : undefined,
                }}
              >
                <button
                  type="button"
                  aria-expanded={isOpen}
                  onClick={() => toggle(g.id)}
                  style={{
                    width: "100%",
                    display: "flex",
                    alignItems: "center",
                    justifyContent: "space-between",
                    gap: 10,
                    padding: "12px 16px",
                    background: "transparent",
                    border: "none",
                    color: "var(--text)",
                    textAlign: "left",
                    cursor: "pointer",
                    fontSize: "var(--font-base)",
                    fontWeight: 600,
                  }}
                >
                  <span style={{ display: "flex", alignItems: "center", gap: 8 }}>
                    {isOpen ? <ChevronDown size={16} aria-hidden="true" /> : <ChevronRight size={16} aria-hidden="true" />}
                    <span>{g.title}</span>
                  </span>
                  <span style={{ fontSize: "var(--font-xs)", fontWeight: 500, color: "var(--text-muted)" }}>
                    {g.examples.length}
                  </span>
                </button>
                {isOpen && (
                  <div style={{ padding: "0 16px 14px 40px", display: "flex", flexDirection: "column", gap: 8 }}>
                    <div style={{ fontSize: "var(--font-sm)", color: "var(--text-muted)" }}>{g.desc}</div>
                    <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
                      {g.examples.map((ex, i) => (
                        <div
                          key={`${g.id}-${ex.tool}-${i}`}
                          style={{
                            display: "flex",
                            alignItems: "center",
                            justifyContent: "space-between",
                            flexWrap: "wrap",
                            gap: 8,
                            padding: "6px 10px",
                            borderRadius: "var(--r-control)",
                            background: "var(--bg-subtle, rgba(128,128,128,0.08))",
                            fontSize: "var(--font-sm)",
                          }}
                        >
                          <span>«{isRu ? ex.ru : ex.en}»</span>
                          <span style={{ display: "flex", gap: 6, flexWrap: "wrap" }}>
                            {ex.badges.map((b) => (
                              <span
                                key={b}
                                style={{
                                  fontSize: "11px",
                                  padding: "2px 7px",
                                  borderRadius: 999,
                                  border: "1px solid var(--border)",
                                  color: b === "confirm" ? "var(--danger, #ef4444)" : "var(--text-muted)",
                                  background: "var(--bg-surface)",
                                }}
                              >
                                {t(BADGE_KEYS[b])}
                              </span>
                            ))}
                          </span>
                        </div>
                      ))}
                    </div>
                  </div>
                )}
              </div>
            );
          })}
        </Group>
      )}
    </div>
  );
}
