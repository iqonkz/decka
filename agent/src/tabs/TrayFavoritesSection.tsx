import { useState } from "react";
import { CATALOG, getLanguage, pickText, t } from "@decka/shared";
import Checkbox from "../ui/Checkbox";
import EntryIcon from "../ui/EntryIcon";
import { Group, Row, SectionHeader } from "../ui/SettingsGroup";

const FAV_STORAGE_KEY = "decka.tray.favorites.v1";
export const MAX_TRAY_FAVORITES = 6;
const DEFAULT_FAVS = ["media.play", "sound.mute", "sound.mic", "win.screenshot", "win.lock", "sound.note"];

export function loadTrayFavorites(): string[] {
  try {
    const raw = window.localStorage.getItem(FAV_STORAGE_KEY);
    if (!raw) return DEFAULT_FAVS;
    const parsed = JSON.parse(raw);
    return Array.isArray(parsed) ? parsed.slice(0, MAX_TRAY_FAVORITES) : DEFAULT_FAVS;
  } catch {
    return DEFAULT_FAVS;
  }
}

function saveTrayFavorites(ids: string[]): void {
  try {
    window.localStorage.setItem(FAV_STORAGE_KEY, JSON.stringify(ids));
  } catch {
    // Ignore storage unavailability in SSR tests.
  }
}

const TRAY_CANDIDATES = CATALOG.items.filter((i) => i.action !== undefined).slice(0, 12);

export interface TrayFavoritesSectionProps {
  initialFavorites?: string[];
}

/** M32 T32.1b: Select up to 6 favorite actions for the tray mini-panel. */
export default function TrayFavoritesSection({ initialFavorites }: TrayFavoritesSectionProps = {}) {
  const [selected, setSelected] = useState<string[]>(() => (initialFavorites ?? loadTrayFavorites()).slice(0, MAX_TRAY_FAVORITES));
  const lang = getLanguage();

  function toggle(id: string, on: boolean) {
    setSelected((cur) => {
      let next: string[];
      if (on) {
        if (cur.includes(id) || cur.length >= MAX_TRAY_FAVORITES) return cur;
        next = [...cur, id];
      } else {
        next = cur.filter((x) => x !== id);
      }
      if (!initialFavorites) saveTrayFavorites(next);
      return next;
    });
  }

  return (
    <div id="section-tray-favorites" style={{ marginTop: 24 }}>
      <SectionHeader
        title={t("m32.tray_favs_title")}
        subtitle={`${t("m32.tray_favs_hint")} · ${t("m32.tray_favs_count", { n: selected.length })}`}
      />
      <Group>
        {TRAY_CANDIDATES.map((item) => {
          const checked = selected.includes(item.id);
          const title = pickText(item.name, lang);
          return (
            <Row
              key={item.id}
              title={
                <span style={{ display: "inline-flex", alignItems: "center", gap: 8 }}>
                  <EntryIcon icon={item.icon} size={18} />
                  <span>{title}</span>
                </span>
              }
            >
              <Checkbox
                checked={checked}
                disabled={!checked && selected.length >= MAX_TRAY_FAVORITES}
                aria-label={title}
                onChange={(on) => toggle(item.id, on)}
              />
            </Row>
          );
        })}
      </Group>
    </div>
  );
}
