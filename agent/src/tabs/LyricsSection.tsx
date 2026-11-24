import { t } from "@decka/shared";
import { Group, PageHeader, Row } from "../ui/SettingsGroup";
import Switch from "../ui/Switch";

/** Detail view for `More -> Integrations -> Lyrics` (DATA_MODEL.md 14). */
export default function LyricsSection({
  enabled,
  onToggle,
}: {
  enabled: boolean;
  onToggle: (next: boolean) => void;
}) {
  return (
    <div id="section-integrations-lyrics">
      <PageHeader icon="music-notes" title={t("integ.item_lyrics_name")} />
      <Group>
        <Row title={t("integ.lyrics_toggle")} description={t("integ.item_lyrics_desc")} htmlFor="lyrics-enabled-switch">
          <Switch id="lyrics-enabled-switch" checked={enabled} onChange={onToggle} label={t("integ.item_lyrics_name")} />
        </Row>
      </Group>
    </div>
  );
}
