import { renderToStaticMarkup } from "react-dom/server";
import { afterAll, beforeAll, describe, expect, test, vi } from "vitest";
import type { AiInfo, LayoutItem, Page, TwitchInfo, YoutubeInfo } from "@decka/shared";
import { DeckEnvContext, EMPTY_DATA, type DeckData, type DeckEnv } from "../src/deckEnv";
import { DeckMetricsContext } from "../src/metricsContext";
import Tile from "../src/Tile";

// The markup of the widgets for fixed data: a change that breaks a widget (or changes it on purpose) shows as a diff here
// without a phone. The look of a theme is CSS and is not part of the markup.
const NOW = Date.UTC(2026, 10, 18, 12, 0, 0);
beforeAll(() => {
  vi.useFakeTimers();
  vi.setSystemTime(NOW);
});
afterAll(() => vi.useRealTimers());

const NOOP = () => {};
const render = (item: LayoutItem, data: Partial<DeckData> = {}) => {
  const env: DeckEnv = { ...EMPTY_DATA, online: true, ...data, onPress: () => Promise.resolve(), onPressError: NOOP, onNavigate: NOOP, onDismiss: NOOP };
  return renderToStaticMarkup(
    <DeckEnvContext.Provider value={env}>
      <DeckMetricsContext.Provider value={{ side: 100, gap: 4 }}>
        <Tile item={item} />
      </DeckMetricsContext.Provider>
    </DeckEnvContext.Provider>,
  );
};
const at = (kind: string, w: number, h: number, extra: object = {}) => ({ id: "w1", kind, x: 0, y: 0, w, h, ...extra }) as LayoutItem;

const ai: AiInfo = {
  claude: { windows: [{ label: "5h", pct: 62, resetsAt: NOW + 3_600_000 }, { label: "7d", pct: 91, resetsAt: NOW + 86_400_000 }], plan: "Max", updatedAt: NOW },
  codex: { windows: [{ label: "30d", pct: 4, resetsAt: NOW + 86_400_000 * 20 }], updatedAt: NOW },
  sessions: [{ id: "abc12345", project: "decka", model: "Opus", ctxPct: 41, lastSeen: NOW }],
  pending: [],
  history: [],
};
const twitch: TwitchInfo = { connected: true, login: "streamer", live: true, viewers: 42, followers: 1234, title: "Speedrun", game: "Celeste", startedAt: NOW - 3_600_000 };
const youtube: YoutubeInfo = { connected: true, channel: "My channel", live: false, viewers: 0, subscribers: 1500 };

describe("widget markup", () => {
  test("AI limits: both providers, one provider at 2x1, the waiting badge", () => {
    expect(render(at("ai", 3, 2), { ai })).toMatchSnapshot();
    expect(render(at("ai", 2, 1, { show: "claude" }), { ai })).toMatchSnapshot();
    expect(render(at("ai", 2, 1, { show: "codex" }), { ai })).toMatchSnapshot();
    const waiting: AiInfo = { ...ai, pending: [{ reqId: "0123456789abcdef", provider: "claude", tool: "Bash", summary: "git status", project: "decka", sessionId: "abc12345", expiresAt: NOW + 60_000 }] };
    expect(render(at("ai", 3, 2), { ai: waiting })).toContain("ai-badge");
    expect(render(at("ai", 2, 1), { ai: null })).toMatchSnapshot();
  });

  test("Twitch and YouTube: live and not live", () => {
    expect(render(at("twitch", 3, 1), { twitch })).toMatchSnapshot();
    expect(render(at("youtube", 3, 1), { youtube })).toMatchSnapshot();
  });

  test("App keys: the keys of the page of the program in front, and the hint without one", () => {
    const page: Page = {
      id: "mail",
      name: "Outlook",
      apps: ["outlook.exe"],
      items: [
        { id: "a", kind: "button", x: 0, y: 0, w: 1, h: 1, label: "New", icon: "lucide:plus" },
        { id: "b", kind: "button", x: 1, y: 0, w: 1, h: 1, label: "Reply", icon: "lucide:mail" },
        { id: "home", kind: "button", x: 6, y: 3, w: 1, h: 1, label: "Home", navigate: "main" },
      ],
    };
    const html = render(at("context", 2, 1), { pages: [page], focusApp: "outlook.exe" });
    expect(html).toMatchSnapshot();
    expect(html).not.toContain("Home");
    expect(render(at("context", 2, 1), { pages: [page], focusApp: "notepad.exe" })).toContain("notepad.exe");
  });

  test("a key with a label and an icon", () => {
    expect(render(at("button", 1, 1, { label: "Mute", icon: "lucide:mic-off" }))).toMatchSnapshot();
  });
});
