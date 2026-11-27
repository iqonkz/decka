import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { setLanguage, type ConfigFile } from "@decka/shared";
import WakeWordBlock from "../src/tabs/WakeWordBlock";
import CalendarSection from "../src/tabs/CalendarSection";
import TrayFavoritesSection from "../src/tabs/TrayFavoritesSection";
import VoiceCommands from "../src/tabs/VoiceCommands";
import { buildIntegrationsList } from "../src/tabs/integrationsCatalog";

const sampleConfig: ConfigFile = {
  version: 1,
  agentName: "PC",
  port: 9247,
  bind: "0.0.0.0",
  apiToken: "dk_test",
  autostart: false,
  usb: { enabled: false, adbPath: "adb" },
  collectors: { sys: { enabled: true, intervalSec: 2 }, checks: [] },
  obs: { enabled: false, port: 4455 },
  twitch: { enabled: false, clientId: "" },
  youtube: { enabled: false, clientId: "" },
  mail: { accounts: [] },
  windowsNotifications: { enabled: false, exclude: [] },
  weather: { enabled: false, place: "", lat: 0, lon: 0, units: "metric" },
  ai: { claude: { enabled: false, approvals: true, timeoutSec: 60 }, codex: { enabled: false } },
};

describe("M32 T32.1b PC settings (wake word, calendar, tray favorites, voice help)", () => {
  it("renders WakeWordBlock with privacy notice, 3 sensitivity levels, and indicator test", () => {
    setLanguage("ru");
    const html = renderToStaticMarkup(
      <WakeWordBlock initialWakeWord={{ enabled: true, sensitivity: "medium", testPhase: "hit" }} />,
    );
    expect(html).toContain("Слово-пробуждение «Декка»");
    expect(html).toContain("звук не сохраняется");
    expect(html).toContain("Низкая");
    expect(html).toContain("Средняя");
    expect(html).toContain("Высокая");
    expect(html).toContain("Скажите «Декка»");
    expect(html).toContain("Сработало: «Декка»");
  });

  it("renders CalendarSection with ICS URL field and upcoming events + calendar card in catalog", () => {
    setLanguage("ru");
    const list = buildIntegrationsList(sampleConfig, 0, false);
    expect(list.some((i) => i.id === "calendar")).toBe(true);

    const html = renderToStaticMarkup(
      <CalendarSection
        initialStatus={{
          url: "https://calendar.example/basic.ics",
          connected: true,
          events: [
            { id: "e1", startTime: "14:00", title: "Team Sync", meetingProvider: "Google Meet", joinUrl: "https://meet.example" },
          ],
        }}
      />,
    );
    expect(html).toContain("Календарь (ICS)");
    expect(html).toContain("Windows DPAPI");
    expect(html).toContain("Team Sync");
    expect(html).toContain("Подключиться");
  });

  it("renders TrayFavoritesSection capped at 6 items", () => {
    setLanguage("ru");
    const html = renderToStaticMarkup(
      <TrayFavoritesSection initialFavorites={["media.play", "sound.mute", "sound.mic", "win.screenshot", "win.lock", "sound.note"]} />,
    );
    expect(html).toContain("Избранные действия мини-панели");
    expect(html).toContain("Выбрано 6 из 6");
  });

  it("includes calendar phrases and wake word hint in VoiceCommands", () => {
    setLanguage("ru");
    const html = renderToStaticMarkup(<VoiceCommands initialQuery="встреча" />);
    expect(html).toContain("Скажите «Декка, …»");
    expect(html).toContain("Когда следующая встреча?");
    setLanguage("en");
  });
});
