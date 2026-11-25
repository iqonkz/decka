import type { LyricsPcOverlay } from "@decka/shared";
import { t } from "@decka/shared";
import { Group, PageHeader, Row } from "../ui/SettingsGroup";
import Segmented from "../ui/Segmented";
import Switch from "../ui/Switch";
/** Detail view for `More -> Integrations -> Lyrics` (DATA_MODEL.md 14, DESIGN.md 4.17). */
export default function LyricsSection({
  enabled,
  onToggle,
  pcOverlay,
  onPcOverlay,
}: {
  enabled: boolean;
  onToggle: (next: boolean) => void;
  pcOverlay: LyricsPcOverlay;
  onPcOverlay: (mode: LyricsPcOverlay) => void;
}) {
  return (
    <div id="section-integrations-lyrics">
      <PageHeader icon="music-notes" title={t("integ.item_lyrics_name")} />
      <Group>
        <Row title={t("integ.lyrics_toggle")} description={t("integ.item_lyrics_desc")} htmlFor="lyrics-enabled-switch">
          <Switch id="lyrics-enabled-switch" checked={enabled} onChange={onToggle} label={t("integ.item_lyrics_name")} />
        </Row>
        {enabled && (
          <Row title={t("integ.lyrics_pc_title")} description={t("integ.lyrics_pc_desc")}>
            <Segmented<LyricsPcOverlay>
              aria-label={t("integ.lyrics_pc_title")}
              options={[
                { value: "off", label: t("integ.lyrics_pc_off") },
                { value: "auto", label: t("integ.lyrics_pc_auto") },
                { value: "always", label: t("integ.lyrics_pc_always") },
              ]}
              value={pcOverlay}
              onChange={onPcOverlay}
            />
          </Row>
        )}
      </Group>
    </div>
  );
}
