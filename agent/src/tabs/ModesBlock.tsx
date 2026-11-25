import { useCallback, useEffect, useState } from "react";
import { t } from "@decka/shared";
import type { WorkMode } from "@decka/ui";
import { invoke } from "@tauri-apps/api/core";
import Button from "../ui/Button";
import Modal from "../ui/Modal";
import PhIcon from "../ui/PhIcon";
import { Group, Row, SectionHeader, SettingsEmpty } from "../ui/SettingsGroup";

export interface ModesBlockProps {
  /** Preview / test seed; when omitted, queries optional `modes_get` command. */
  initialModes?: WorkMode[];
  onSaveMode?: (mode: WorkMode, originalName?: string) => void;
  onDeleteMode?: (name: string) => void;
}

/**
 * `More -> Voice -> Work Modes` (TASKS.md §T27.10 / §T27.2b): inspect, add, edit apps/focus duration, and remove modes.
 */
export default function ModesBlock({ initialModes, onSaveMode, onDeleteMode }: ModesBlockProps) {
  const [modes, setModes] = useState<WorkMode[]>(initialModes ?? []);
  const [editing, setEditing] = useState<{
    originalName?: string;
    name: string;
    appsText: string;
    focusText: string;
  } | null>(null);

  const load = useCallback(() => {
    if (initialModes) return;
    invoke<WorkMode[]>("modes_list").then(
      (rows) => Array.isArray(rows) && setModes(rows),
      () => {},
    );
  }, [initialModes]);

  useEffect(() => {
    load();
  }, [load]);

  const openNew = () => setEditing({ name: "", appsText: "", focusText: "" });
  const openEdit = (m: WorkMode) =>
    setEditing({
      originalName: m.name,
      name: m.name,
      appsText: m.apps.map((a) => a.exe).join(", "),
      focusText: m.focusMinutes ? String(m.focusMinutes) : "",
    });

  const saveEdit = () => {
    if (!editing) return;
    const name = editing.name.trim();
    if (!name) return;
    const apps = editing.appsText
      .split(",")
      .map((s) => s.trim())
      .filter(Boolean)
      .map((exe) => ({ exe }));
    const parsedFocus = Number(editing.focusText.trim());
    const focusMinutes = Number.isFinite(parsedFocus) && parsedFocus > 0 ? Math.round(parsedFocus) : undefined;
    const prevMode = modes.find((m) => m.name === editing.originalName);
    const nextMode: WorkMode = {
      name,
      apps: apps.length > 0 ? apps : prevMode?.apps ?? [],
      ...(focusMinutes !== undefined ? { focusMinutes } : {}),
    };
    setModes((prev) => {
      if (editing.originalName) {
        return prev.map((m) => (m.name === editing.originalName ? nextMode : m));
      }
      return [...prev, nextMode];
    });
    onSaveMode?.(nextMode, editing.originalName);
    if (!initialModes) {
      invoke("modes_save", { mode: nextMode, originalName: editing.originalName }).catch(() => {});
    }
    setEditing(null);
  };

  const removeMode = (name: string) => {
    setModes((prev) => prev.filter((m) => m.name !== name));
    onDeleteMode?.(name);
    if (!initialModes) {
      invoke("modes_delete", { name }).catch(() => {});
    }
  };

  return (
    <div id="voice-modes" style={{ marginTop: 24 }}>
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 12 }}>
        <SectionHeader id="section-voice-modes" title={t("m27.modes_title")} subtitle={t("m27.modes_hint")} />
        <Button onClick={openNew}>
          <PhIcon name="plus" size={14} /> {t("m27.mode_add")}
        </Button>
      </div>
      {modes.length === 0 ? (
        <SettingsEmpty>{t("m27.modes_empty")}</SettingsEmpty>
      ) : (
        <Group>
          {modes.map((m) => {
            const appList = m.apps.map((a) => a.exe).join(", ") || "—";
            const focusPart = m.focusMinutes ? ` · ${t("m27.mode_focus_min", { min: m.focusMinutes })}` : "";
            return (
              <Row key={m.name} title={m.name} description={`${appList}${focusPart}`}>
                <Button
                  className="btn-icon"
                  aria-label={t("m27.mode_edit")}
                  title={t("m27.mode_edit")}
                  onClick={() => openEdit(m)}
                >
                  <PhIcon name="pencil-simple" size={16} />
                </Button>
                <Button
                  className="btn-icon"
                  aria-label={t("common.delete")}
                  title={t("common.delete")}
                  onClick={() => removeMode(m.name)}
                >
                  <PhIcon name="trash" size={16} />
                </Button>
              </Row>
            );
          })}
        </Group>
      )}

      {editing && (
        <Modal
          title={editing.originalName ? t("m27.mode_edit") : t("m27.mode_add")}
          onClose={() => setEditing(null)}
          footer={
            <>
              <Button onClick={() => setEditing(null)}>{t("common.cancel")}</Button>
              <Button variant="primary" disabled={!editing.name.trim()} onClick={saveEdit}>
                {t("m27.mode_save")}
              </Button>
            </>
          }
        >
          <div className="stack" style={{ gap: 12 }}>
            <label className="stack" style={{ gap: 4 }}>
              <span>{t("m27.mode_name")}</span>
              <input
                value={editing.name}
                maxLength={60}
                onChange={(e) => setEditing({ ...editing, name: e.target.value })}
              />
            </label>
            <label className="stack" style={{ gap: 4 }}>
              <span>{t("m27.mode_apps")}</span>
              <input
                value={editing.appsText}
                placeholder={t("m27.mode_apps")}
                onChange={(e) => setEditing({ ...editing, appsText: e.target.value })}
              />
            </label>
            <label className="stack" style={{ gap: 4 }}>
              <span>{t("m27.mode_focus")}</span>
              <input
                type="number"
                min={0}
                max={240}
                value={editing.focusText}
                onChange={(e) => setEditing({ ...editing, focusText: e.target.value })}
              />
            </label>
          </div>
        </Modal>
      )}
    </div>
  );
}
