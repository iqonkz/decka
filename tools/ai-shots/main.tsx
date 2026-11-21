// Harness of tools/ai-shots.mjs: draws the real deck Grid with one AI limits tile (T20.2). Not part of the app.
import { createRoot } from "react-dom/client";
import { Grid, applyTheme, EMPTY_DATA, resetAiHistory, type DeckData } from "@decka/ui";
import { setLanguage, type AiInfo, type Language, type LayoutItem, type ThemeId } from "@decka/shared";

const q = new URLSearchParams(location.search);
const w = Number(q.get("w") ?? 2);
const h = Number(q.get("h") ?? 1);
const theme = (q.get("theme") ?? "amoled") as ThemeId;
const show = (q.get("show") ?? "both") as "both" | "claude" | "codex";
setLanguage((q.get("lang") ?? "en") as Language);
resetAiHistory();
applyTheme(document.documentElement, { theme, accent: "#7c5cff", wallpaper: "aurora" });

const now = Date.now();
const ai: AiInfo = {
  claude: { windows: [{ label: "5h", pct: 62, resetsAt: now + 134 * 60000 }, { label: "7d", pct: 91, resetsAt: now + 3 * 86400000 }], updatedAt: now },
  codex: { windows: [{ label: "5h", pct: 18, resetsAt: now + 41 * 60000 }, { label: "7d", pct: 74, resetsAt: now + 5 * 86400000 }], plan: "Plus", updatedAt: now },
  sessions: [
    { id: "a1", project: "decka", model: "Opus", ctxPct: 34, lastSeen: now },
    { id: "a2", project: "billing-service", model: "Sonnet", ctxPct: 71, lastSeen: now },
    { id: "a3", project: "site", lastSeen: now },
  ],
  pending: [],
  history: [],
};
const item = { id: "ai1", kind: "ai", x: 0, y: 0, w, h, show } as LayoutItem;
const data: DeckData = { ...EMPTY_DATA, online: true, ai: q.get("nodata") ? null : ai };
const noop = () => {};
createRoot(document.getElementById("root")!).render(
  <Grid
    page={{ id: "p", name: "p", items: [item] }}
    grid={{ cols: 7, rows: 4 }}
    data={data}
    onPress={() => Promise.resolve()}
    onPressError={noop}
    onNavigate={noop}
    onDismiss={noop}
  />,
);
