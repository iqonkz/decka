import { renderToStaticMarkup } from "react-dom/server";
import { beforeEach, describe, expect, test } from "vitest";
import { setLanguage } from "@decka/shared";
import Screensaver from "../src/display/Screensaver";
import Toasts from "../src/deck/Toasts";
import PhoneMenu from "../src/menu/PhoneMenu";
import PairScreen from "../src/screens/PairScreen";
import { pushToast, resetState, setState } from "../src/store/store";

describe("T31.8b phone surrounding screens", () => {
  beforeEach(() => {
    resetState();
    setLanguage("ru");
    // Browser location.host stub for SSR PairScreen test
    if (typeof (globalThis as { location?: unknown }).location === "undefined") {
      (globalThis as { location: { host: string } }).location = { host: "192.168.1.20:9247" };
    }
  });

  test("PhoneMenu renders PC-matching section titles and 20 px Lucide icons", () => {
    const html = renderToStaticMarkup(
      <PhoneMenu reduced={false} onToggleReduced={() => {}} haptics onToggleHaptics={() => {}} onUnpair={() => {}} onClose={() => {}} />,
    );
    expect(html).toContain("phone-group-title");
    expect(html).toContain("Подключения");
    expect(html).toContain("Настройки");
    expect(html).toContain("Приложение");
    expect(html).toContain("phone-row");
    expect(html).toContain("<svg");
  });

  test("PairScreen in a browser renders the code form and the word error, and no scanner frame", () => {
    setState({ lastError: "Код неверный или устарел. Создайте новый на ПК." });
    const html = renderToStaticMarkup(<PairScreen />);
    expect(html).toContain("pair-fullscreen");
    expect(html).not.toContain("pair-viewfinder");
    expect(html).toContain("pair-bottom-sheet");
    expect(html).toContain("pair-error-banner");
    expect(html).toContain("Код неверный или устарел");
  });

  test("Toasts renders at most 2 items with typed icon", () => {
    pushToast({ level: "info", title: "One" });
    pushToast({ level: "warning", title: "Two" });
    pushToast({ level: "error", title: "Three" });
    const html = renderToStaticMarkup(<Toasts />);
    expect(html).not.toContain("One");
    expect(html).toContain("Two");
    expect(html).toContain("Three");
  });

  test("Screensaver renders tabular clock, date, and active track", () => {
    setState({
      conn: "online",
      state: {
        layout: { version: 1, grid: { cols: 7, rows: 4 }, display: { dimAfterSec: 60, dimLevel: 0.3, sleepAfterSec: 600, pixelShift: true, night: { enabled: false, from: "23:00", to: "07:00" } }, pages: [] },
        sys: null,
        statuses: [],
        notifications: [],
        vars: {},
        icons: {},
        focus: null,
        media: { playing: true, title: "Midnight City", artist: "M83", posMs: 1000, durMs: 120000 },
        lyrics: null,
        weather: null,
        obs: null,
        ai: null,
        twitch: null,
        youtube: null,
        strip: null,
        battery: null,
        mail: null,
        reminders: [],
        lists: [],
        update: null,
      } as never,
    });
    const html = renderToStaticMarkup(<Screensaver />);
    expect(html).toContain("saver-time");
    expect(html).toContain("saver-date");
    expect(html).toContain("M83 — Midnight City");
  });
});
