import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";
import { setLanguage, type CalendarStatus, type ConfigFile, type TrayState, type WakeStatus } from "@decka/shared";
import WakeWordBlock from "../src/tabs/WakeWordBlock";
import CalendarSection from "../src/tabs/CalendarSection";
import TrayFavoritesSection, { favoriteCandidates } from "../src/tabs/TrayFavoritesSection";
import VoiceCommands from "../src/tabs/VoiceCommands";
import { buildIntegrationsList } from "../src/tabs/integrationsCatalog";
import { M32_STORAGE_KEYS, removeM32Storage } from "../src/m32Cleanup";
import { panelData } from "../src/TrayApp";

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

const wake: WakeStatus = { enabled: true, sensitivity: "medium", installed: true, state: "listening", reason: null, percent: null, heard: 3, testing: false, size: 36_880_000, silent: false };

const calendar: CalendarStatus = {
  connected: true,
  host: "calendar.google.com",
  masked: "https://calendar.google.com/••••••••",
  info: {
    updatedAt: 1,
    events: [{ id: "cal-1", title: "Team Sync", startTime: "14:00", endTime: "14:30", day: "2026-11-27", start: 1, end: 2, allDay: false, meetingProvider: "Google Meet", joinActionId: "cal-join-1" }],
  },
};

describe("M32 settings (wake word, calendar, tray favorites, voice help)", () => {
  it("shows the privacy notice, three sensitivity levels and a detection of the test", () => {
    setLanguage("ru");
    const html = renderToStaticMarkup(<WakeWordBlock initialStatus={wake} initialTestFrom={2} />);
    for (const text of ["Слово-пробуждение «Декка»", "звук не сохраняется", "Низкая", "Средняя", "Высокая", "Скажите «Декка»", "Сработало: «Декка»"]) expect(html).toContain(text);
    const waiting = renderToStaticMarkup(<WakeWordBlock initialStatus={{ ...wake, state: "paused" }} />);
    expect(waiting).not.toContain("Сработало");
    expect(waiting).toContain("Пауза");
    expect(html).toContain("Слушает: скажите «Декка»");
    expect(renderToStaticMarkup(<WakeWordBlock initialStatus={{ ...wake, silent: true }} />)).toContain("микрофон передаёт тишину");
    expect(renderToStaticMarkup(<WakeWordBlock initialStatus={{ ...wake, state: "error", reason: "No microphone found" }} />)).toContain("Не запущено: No microphone found");
  });

  it("names the wake word alone, in every language, and starts at the middle level (T34.5)", () => {
    const dictionaries = import.meta.glob("../../shared/src/i18n/*.ts", { query: "?raw", import: "default", eager: true }) as Record<string, string>;
    expect(Object.keys(dictionaries).length).toBeGreaterThanOrEqual(11);
    for (const [file, text] of Object.entries(dictionaries)) {
      expect(text, file).not.toMatch(/(?<![a-zа-я])(hey|hi|эй|хей|әй|嘿)[,，、]?\s*[«"„“]?(decka|декка)/iu);
    }
    setLanguage("ru");
    expect(renderToStaticMarkup(<WakeWordBlock />)).toMatch(/aria-checked="true"[^>]*>Средняя</);
  });

  it("shows the saved link only masked and the upcoming events", () => {
    setLanguage("ru");
    expect(buildIntegrationsList({ ...sampleConfig, calendar: { url: "" } }, 0, false).find((i) => i.id === "calendar")?.connected).toBe(true);
    expect(buildIntegrationsList(sampleConfig, 0, false).find((i) => i.id === "calendar")?.connected).toBe(false);
    const html = renderToStaticMarkup(<CalendarSection initialStatus={calendar} />);
    for (const text of ["Календарь (ICS)", "Windows DPAPI", "••••••••", "Team Sync", "Подключиться", "27.11 · 14:00 · Google Meet"]) expect(html).toContain(text);
    expect(html).toContain('type="password"');
    expect(html).not.toContain(".ics");
  });

  it("keeps nothing of the link in the window: the drafts' keys are removed and nothing writes them", async () => {
    const data = new Map<string, string>([["decka.calendar.v1", JSON.stringify({ url: "https://x/secret-123.ics" })], ["decka.wakeword.v1", "{}"], ["decka.tray.favorites.v1", "[]"], ["decka.other", "stays"]]);
    removeM32Storage({ removeItem: (k) => void data.delete(k) });
    expect([...data.keys()]).toEqual(["decka.other"]);
    // The sources of the window never name these keys again, except the file that removes them.
    const sources = import.meta.glob("../src/**/*.{ts,tsx}", { query: "?raw", import: "default", eager: true }) as Record<string, string>;
    for (const [file, text] of Object.entries(sources)) {
      if (!file.endsWith("m32Cleanup.ts")) for (const key of M32_STORAGE_KEYS) expect(text, file).not.toContain(key);
      if (/CalendarSection|WakeWordBlock|TrayFavoritesSection/.test(file)) expect(text, file).not.toContain("localStorage.");
    }
    vi.unstubAllGlobals();
  });

  it("offers the actions of the PC as favourites, six at most", () => {
    setLanguage("ru");
    const candidates = favoriteCandidates(
      { version: 1, actions: { lock: { type: "system", name: "Lock", op: "lock" } } },
      { version: 1, grid: { cols: 7, rows: 4 }, display: {} as never, pages: [{ id: "main", name: "Main", items: [{ id: "b", kind: "button", x: 0, y: 0, w: 1, h: 1, actionId: "lock", label: "Блок", icon: "lucide:lock" }] }] },
    );
    expect(candidates).toEqual([{ id: "lock", label: "Блок", icon: "lucide:lock" }]);
    const six = Array.from({ length: 7 }, (_, i) => ({ id: `a${i}`, label: `A${i}`, icon: "lucide:zap" }));
    const html = renderToStaticMarkup(<TrayFavoritesSection initial={{ candidates: six, favorites: six.slice(0, 6).map((c) => c.id) }} />);
    expect(html).toContain("Избранные действия мини-панели");
    expect(html).toContain("Выбрано 6 из 6");
  });

  it("turns the answer of tray_state into the panel, with a second press for a key that asks", () => {
    setLanguage("ru");
    const state: TrayState = { media: null, volume: 40, muted: false, micMuted: true, favorites: [{ id: "off", label: "Выключить", icon: "lucide:power", active: false, confirm: true }], nextEvent: calendar.info!.events[0]!, icons: {} };
    expect(panelData(state, null).favorites[0]?.label).toBe("Выключить");
    expect(panelData(state, "off").favorites[0]).toMatchObject({ label: "Ещё раз", active: true });
    expect(panelData(state, null).nextEvent).toEqual({ title: "Team Sync", startTime: "14:00", joinActionId: "cal-join-1" });
    expect(panelData(null, null)).toMatchObject({ track: null, volume: 0, favorites: [], nextEvent: null });
  });

  it("includes the calendar phrases and the hint of the wake word in the voice help", () => {
    setLanguage("ru");
    const html = renderToStaticMarkup(<VoiceCommands initialQuery="встреча" />);
    expect(html).toContain("Скажите «Декка, …»");
    expect(html).toContain("Когда следующая встреча?");
    setLanguage("en");
  });
});
