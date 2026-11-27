import { useEffect, useState } from "react";
import { t, type ActionsFile, type LayoutFile } from "@decka/shared";
import { call } from "../ipc";
import Banner from "../ui/Banner";
import Checkbox from "../ui/Checkbox";
import EntryIcon from "../ui/EntryIcon";
import { Group, Row, SectionHeader } from "../ui/SettingsGroup";

export const MAX_TRAY_FAVORITES = 6;

export interface FavoriteCandidate {
  id: string;
  label: string;
  icon: string;
}

/** The actions of the PC as they can be chosen: named and drawn like the key of the deck that runs them. */
export function favoriteCandidates(actions: ActionsFile, layout: LayoutFile): FavoriteCandidate[] {
  const keys = layout.pages.flatMap((p) => p.items).filter((i) => i.kind === "button");
  return Object.entries(actions.actions).map(([id, action]) => {
    const key = keys.find((k) => k.actionId === id);
    return { id, label: key?.label || action.name, icon: key?.icon ?? "lucide:zap" };
  });
}

export interface TrayFavoritesSectionProps {
  /** Tests give what the agent would answer. */
  initial?: { candidates: FavoriteCandidate[]; favorites: string[] };
}

/** Settings: up to six actions of the mini panel of the tray (T32.3), kept by the agent. */
export default function TrayFavoritesSection({ initial }: TrayFavoritesSectionProps = {}) {
  const [candidates, setCandidates] = useState<FavoriteCandidate[]>(initial?.candidates ?? []);
  const [selected, setSelected] = useState<string[]>(initial?.favorites ?? []);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (initial) return;
    let live = true;
    Promise.all([call("get_actions"), call("get_layout"), call("tray_favorites_get")]).then(
      ([actions, layout, chosen]) => {
        if (!live) return;
        setCandidates(favoriteCandidates(actions, layout));
        setSelected(chosen.favorites);
      },
      (e) => {
        if (live) setError(String(e));
      },
    );
    return () => {
      live = false;
    };
  }, [initial]);

  function toggle(id: string, on: boolean) {
    const next = on ? [...selected.filter((x) => x !== id), id].slice(0, MAX_TRAY_FAVORITES) : selected.filter((x) => x !== id);
    setSelected(next);
    setError(null);
    call("tray_favorites_set", { favorites: next }).catch((e) => {
      setSelected(selected); // not saved: show what the agent still has
      setError(String(e));
    });
  }

  return (
    <div id="section-tray-favorites" style={{ marginTop: 24 }}>
      <SectionHeader title={t("m32.tray_favs_title")} subtitle={`${t("m32.tray_favs_hint")} · ${t("m32.tray_favs_count", { n: selected.length })}`} />
      {error && <Banner kind="error">{error}</Banner>}
      <Group>
        {candidates.map((item) => {
          const checked = selected.includes(item.id);
          return (
            <Row
              key={item.id}
              title={
                <span style={{ display: "inline-flex", alignItems: "center", gap: 8 }}>
                  <EntryIcon icon={item.icon.startsWith("lucide:") ? item.icon : "lucide:zap"} size={18} />
                  <span>{item.label}</span>
                </span>
              }
            >
              <Checkbox checked={checked} disabled={!checked && selected.length >= MAX_TRAY_FAVORITES} aria-label={item.label} onChange={(on) => toggle(item.id, on)} />
            </Row>
          );
        })}
      </Group>
    </div>
  );
}
