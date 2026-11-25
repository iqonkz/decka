import { useCallback, useEffect, useState } from "react";
import { t } from "@decka/shared";
import type { SavedGuide } from "@decka/ui";
import { invoke } from "@tauri-apps/api/core";
import Button from "../ui/Button";
import PhIcon from "../ui/PhIcon";
import { Group, Row, SectionHeader, SettingsEmpty } from "../ui/SettingsGroup";

export interface GuidesBlockProps {
  /** Preview / test seed; when omitted, queries optional `guides_get` command. */
  initialGuides?: SavedGuide[];
  onRename?: (id: string, title: string) => void;
  onDelete?: (id: string) => void;
}

/**
 * `More -> Voice -> Guides` (TASKS.md §T27.6 / §T27.2b): local saved guides with inline rename and delete.
 */
export default function GuidesBlock({ initialGuides, onRename, onDelete }: GuidesBlockProps) {
  const [guides, setGuides] = useState<SavedGuide[]>(initialGuides ?? []);
  const [editing, setEditing] = useState<{ id: string; text: string } | null>(null);

  const load = useCallback(() => {
    if (initialGuides) return;
    invoke<SavedGuide[]>("guides_list").then(
      (rows) => Array.isArray(rows) && setGuides(rows),
      () => {},
    );
  }, [initialGuides]);

  useEffect(() => {
    load();
  }, [load]);

  const commitRename = () => {
    const e = editing;
    setEditing(null);
    if (!e) return;
    const next = e.text.trim();
    if (!next) return;
    setGuides((prev) => prev.map((g) => (g.id === e.id ? { ...g, goal: [next, ...g.goal.slice(1)] } : g)));
    onRename?.(e.id, next);
    if (!initialGuides) {
      invoke("guides_rename", { id: e.id, goal: next }).catch(() => {});
    }
  };

  const removeGuide = (id: string) => {
    setGuides((prev) => prev.filter((g) => g.id !== id));
    onDelete?.(id);
    if (!initialGuides) {
      invoke("guides_delete", { id }).catch(() => {});
    }
  };

  return (
    <div id="voice-guides" style={{ marginTop: 24 }}>
      <SectionHeader id="section-voice-guides" title={t("m27.guides_title")} subtitle={t("m27.guides_hint")} />
      {guides.length === 0 ? (
        <SettingsEmpty>{t("m27.guides_empty")}</SettingsEmpty>
      ) : (
        <Group>
          {guides.map((g) => {
            const label = g.goal[0] ?? g.id;
            const desc = t("m27.guide_meta", { process: g.process, steps: g.steps.length, uses: g.uses });
            return (
              <Row
                key={g.id}
                title={
                  editing?.id === g.id ? (
                    <input
                      autoFocus
                      className="grow"
                      maxLength={120}
                      aria-label={t("m27.guides_title")}
                      value={editing.text}
                      onChange={(e) => setEditing({ id: g.id, text: e.target.value })}
                      onBlur={commitRename}
                      onKeyDown={(e) =>
                        e.key === "Enter" ? commitRename() : e.key === "Escape" ? setEditing(null) : undefined
                      }
                    />
                  ) : (
                    <button
                      type="button"
                      className="link rem-edit grow"
                      onClick={() => setEditing({ id: g.id, text: label })}
                    >
                      {label}
                    </button>
                  )
                }
                description={desc}
              >
                <Button
                  className="btn-icon"
                  aria-label={t("common.delete")}
                  title={t("common.delete")}
                  onClick={() => removeGuide(g.id)}
                >
                  <PhIcon name="trash" size={16} />
                </Button>
              </Row>
            );
          })}
        </Group>
      )}
    </div>
  );
}
