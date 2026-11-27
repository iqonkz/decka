import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, test } from "vitest";
import { setLanguage, type LayoutItem } from "@decka/shared";
import { DeckEnvContext, EMPTY_DATA, type DeckData, type DeckEnv } from "../src/deckEnv";
import { DeckMetricsContext } from "../src/metricsContext";
import Tile from "../src/Tile";
import TrayMiniPanel from "../src/tray/TrayMiniPanel";
import type { CalendarInfo } from "@decka/shared";

const NOOP = () => {};

function renderTile(item: LayoutItem, data: Partial<DeckData> = {}) {
  const env: DeckEnv = {
    ...EMPTY_DATA,
    online: true,
    ...data,
    onPress: () => Promise.resolve(),
    onPressError: NOOP,
    onNavigate: NOOP,
    onDismiss: NOOP,
  };
  return renderToStaticMarkup(
    <DeckEnvContext.Provider value={env}>
      <DeckMetricsContext.Provider value={{ side: 100, gap: 8 }}>
        <Tile item={item} />
      </DeckMetricsContext.Provider>
    </DeckEnvContext.Provider>,
  );
}

const today = new Date().toLocaleDateString("sv-SE");
const timed = { endTime: "", day: today, start: 0, end: 0, allDay: false };
const sampleCal: CalendarInfo = {
  updatedAt: 1,
  events: [
    { ...timed, id: "e1", startTime: "14:00", title: "Release Sync", joinActionId: "cal-join-1", meetingProvider: "Zoom" },
    { ...timed, id: "e2", startTime: "16:30", title: "1:1 Design" },
  ],
};

describe("M32 T32.1b Calendar widget & Tray mini-panel", () => {
  test("renders calendar widget empty state and populated S/M/L blocks with Join action", () => {
    setLanguage("ru");
    const empty = renderTile({ id: "cal0", kind: "calendar", x: 0, y: 0, w: 2, h: 2 } as unknown as LayoutItem, { calendar: null });
    expect(empty).toContain("Встреч пока нет");

    const oneByOne = renderTile({ id: "cal1", kind: "calendar", x: 0, y: 0, w: 1, h: 1 } as unknown as LayoutItem, { calendar: sampleCal });
    expect(oneByOne).toContain("calendar-s-time");
    expect(oneByOne).toContain("14:00");
    expect(oneByOne).not.toContain('class="tile-head"');

    const twoByTwo = renderTile({ id: "cal2", kind: "calendar", x: 0, y: 0, w: 2, h: 2 } as unknown as LayoutItem, { calendar: sampleCal });
    expect(twoByTwo).toContain("Календарь");
    expect(twoByTwo).toContain("Release Sync");
    expect(twoByTwo).toContain("1:1 Design");
    expect(twoByTwo).toContain("Подключиться");
    setLanguage("en");
  });

  test("renders 320x420 TrayMiniPanel in light and dark themes with up to 6 favorites", () => {
    setLanguage("ru");
    const favorites = Array.from({ length: 7 }, (_, i) => ({
      id: `act-${i}`,
      label: `Action ${i + 1}`,
      icon: "lucide:zap",
    }));
    const darkHtml = renderToStaticMarkup(
      <TrayMiniPanel
        theme="dark"
        track={{ title: "Midnight City", artist: "M83", playing: true }}
        volume={64}
        micMuted={false}
        favorites={favorites}
        nextEvent={{ startTime: "14:00", title: "Release Sync", joinActionId: "cal-join-1" }}
      />,
    );
    expect(darkHtml).toContain('data-tray-theme="dark"');
    expect(darkHtml).toContain("Midnight City");
    expect(darkHtml).toContain("64%");
    expect(darkHtml).toContain("Микрофон вкл");
    expect(darkHtml).toContain("Action 6");
    expect(darkHtml).not.toContain("Action 7");
    expect(darkHtml).toContain("Подключиться");
    expect(darkHtml).toContain("Открыть настройки");

    const lightHtml = renderToStaticMarkup(
      <TrayMiniPanel theme="light" micMuted={true} favorites={favorites.slice(0, 6)} nextEvent={null} />,
    );
    expect(lightHtml).toContain('data-tray-theme="light"');
    expect(lightHtml).toContain("Микрофон выкл");
    expect(lightHtml).toContain("Встреч пока нет");
    setLanguage("en");
  });
});
