#!/usr/bin/env node
// Pictures of the voice pill (PC) and of the voice button, bubble and confirmation sheet (phone) for visual acceptance (T21.S).
// Builds tools/voice-shots with Vite, serves it, drives headless Chrome/Edge with the DevTools protocol.
//   node tools/voice-shots.mjs [outDir]      writes pill-*.png (360x160 css px, x3) and phone-*.png (800x360 css px, x2)
import { spawn } from "node:child_process";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { createServer } from "node:http";
import { tmpdir } from "node:os";
import { extname, join, normalize } from "node:path";
import { createRequire } from "node:module";
import { pathToFileURL } from "node:url";

const root = new URL("..", import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, "$1");
const outDir = process.argv[2] ?? join(root, ".tmp", "shots");
const dist = join(root, ".tmp", "voice-shots-dist");
const CHROME = [process.env.CHROME_PATH, "C:/Program Files/Google/Chrome/Application/chrome.exe", "C:/Program Files (x86)/Google/Chrome/Application/chrome.exe", "C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe"].find((p) => p && existsSync(p));
if (!CHROME) throw new Error("Chrome or Edge not found: set CHROME_PATH");

const require = createRequire(join(root, "client", "package.json"));
const { build } = await import(pathToFileURL(require.resolve("vite")).href);
const react = (await import(pathToFileURL(require.resolve("@vitejs/plugin-react")).href)).default;
await build({
  root: join(root, "tools", "voice-shots"),
  resolve: {
    alias: {
      "@decka/shared": join(root, "shared", "src", "index.ts"),
      "@decka/ui": join(root, "ui", "src", "index.ts"),
      "@decka/client": join(root, "client", "src", "index.ts"),
      "@decka/agent": join(root, "agent", "src", "index.ts"),
    },
  },
  configFile: false,
  logLevel: "warn",
  plugins: [react()],
  base: "./",
  build: { outDir: dist, emptyOutDir: true, target: "chrome100" },
});

const MIME = { ".html": "text/html", ".js": "text/javascript", ".css": "text/css", ".svg": "image/svg+xml", ".woff2": "font/woff2" };
const server = createServer((req, res) => {
  const path = normalize(decodeURIComponent((req.url ?? "/").split("?")[0])).replace(/^[\\/]+/, "") || "index.html";
  let file = join(dist, path);
  // The settings window loads Phosphor icons from /phosphor (copied by agent/scripts/copy-phosphor.mjs, not in git).
  if (!existsSync(file) && path.startsWith("phosphor")) file = join(root, "agent", "public", path);
  if (!(file.startsWith(dist) || file.startsWith(join(root, "agent", "public"))) || !existsSync(file)) return res.writeHead(404).end();
  res.writeHead(200, { "content-type": MIME[extname(file)] ?? "application/octet-stream" }).end(readFileSync(file));
});
await new Promise((r) => server.listen(0, "127.0.0.1", r));
const base = `http://127.0.0.1:${server.address().port}/index.html`;

const PILL = { w: 360, h: 160, scale: 3 };
const PHONE = { w: 800, h: 360, scale: 2 };
const DECK = { w: 1280, h: 600, scale: 1 };
function t235bShots() {
  const out = {};
  const land = { w: 844, h: 390, scale: 1 };
  const port = { w: 390, h: 844, scale: 1 };
  for (const theme of ["amoled", "studio", "material", "glass"]) {
    out[`t235b-demo-${theme}-land`] = [land, `scene=deck&theme=${theme}`];
    out[`t235b-demo-${theme}-port`] = [port, `scene=deck&theme=${theme}&orient=portrait`];
  }
  const gridShot = { w: 1100, h: 640, scale: 1 };
  out["t235b-widgets-sml-1"] = [gridShot, "scene=deck&theme=studio&pack=sml1"];
  out["t235b-widgets-sml-2"] = [gridShot, "scene=deck&theme=studio&pack=sml2"];
  out["t235b-widgets-sml-3"] = [gridShot, "scene=deck&theme=studio&pack=sml3"];
  out["t235b-widgets-empty"] = [gridShot, "scene=deck&theme=studio&pack=empty"];
  for (const lang of ["en", "ru"]) {
    out[`t235b-srv-pair-${lang}`] = [port, `scene=service&screen=pair&lang=${lang}`];
    out[`t235b-srv-qr-${lang}`] = [port, `scene=service&screen=qr&lang=${lang}`];
    out[`t235b-srv-link-${lang}`] = [port, `scene=service&screen=link&lang=${lang}`];
    out[`t235b-srv-update-${lang}`] = [port, `scene=service&screen=update&lang=${lang}`];
    out[`t235b-srv-perm-${lang}`] = [port, `scene=service&screen=perm&lang=${lang}`];
    out[`t235b-srv-menu-${lang}`] = [port, `scene=service&screen=menu&lang=${lang}`];
    out[`t235b-srv-skeleton-${lang}`] = [port, `scene=service&screen=skeleton&lang=${lang}`];
  }
  return out;
}
function t24Shots() {
  const out = {};
  const land = { w: 844, h: 390, scale: 1 };
  const port = { w: 390, h: 844, scale: 1 };
  const pc = { w: 1100, h: 800, scale: 1 };
  out["t24-media-sync-studio"] = [land, "scene=deck&theme=studio&pack=m24-lyrics-sync"];
  out["t24-media-sync-glass"] = [land, "scene=deck&theme=glass&pack=m24-lyrics-sync"];
  out["t24-media-plain-studio"] = [land, "scene=deck&theme=studio&pack=m24-lyrics-plain"];
  out["t24-media-plain-glass"] = [land, "scene=deck&theme=glass&pack=m24-lyrics-plain"];
  out["t24-weather-hourly"] = [land, "scene=deck&theme=studio&pack=m24-weather-hourly"];
  out["t24-reminders-phone"] = [land, "scene=deck&theme=studio&pack=m24-rem-repeat"];
  out["t24-reminders-pc"] = [pc, "scene=settings&theme=studio&lang=ru&go=reminders", ".rem-time-pill::0"];
  out["t24-integ-lyrics"] = [pc, "scene=settings&theme=studio&lang=ru&go=integrations"];
  out["t24-focus-switch"] = [pc, "scene=settings&theme=studio&lang=ru&go=look&focus=.settings-switch"];
  out["t24-focus-segmented"] = [pc, "scene=settings&theme=studio&lang=ru&go=look&focus=.segmented-item"];
  out["t24-focus-button"] = [pc, "scene=settings&theme=studio&lang=ru&go=integrations&focus=.btn-primary"];
  for (const step of [0, 1, 2]) {
    for (const scheme of ["light", "dark"]) {
      out[`t24-wizard-${step}-${scheme}`] = [pc, `scene=settings&theme=studio&lang=ru&scheme=${scheme}&wizard=${step}`];
    }
  }
  out["t24-phone-nolayout-ru"] = [port, "scene=deck&theme=studio&lang=ru&pack=m24-empty-deck"];
  return out;
}
function m26Shots() {
  const islandSize = { w: 600, h: 420, scale: 2 };
  const orbRowSize = { w: 760, h: 200, scale: 2 };
  return {
    "m26-orb-states-dark": [orbRowSize, "scene=island&orbRow=1&theme=dark"],
    "m26-orb-states-light": [orbRowSize, "scene=island&orbRow=1&theme=light"],
    "m26-island-tab": [{ w: 300, h: 100, scale: 2 }, "scene=island&mode=tab&theme=dark"],
    "m26-island-compact-lyrics": [{ w: 480, h: 160, scale: 2 }, "scene=island&mode=compact&voiceOff=1&lyrics=1&theme=dark"],
    "m26-island-compact-lyrics-hover": [{ w: 480, h: 160, scale: 2 }, "scene=island&mode=compact&voiceOff=1&lyrics=1&hover=1&theme=dark"],
    "m26-island-compact-voice": [{ w: 480, h: 160, scale: 2 }, "scene=island&mode=compact&state=listening&theme=dark"],
    "m26-island-solo-listening": [{ w: 400, h: 320, scale: 2 }, "scene=island&mode=expanded&state=listening&solo=1&lang=ru&theme=dark"],
    "m26-island-transition-mid": [islandSize, "scene=island&mode=expanded&state=thinking&transition=0.55&lang=ru&theme=dark"],
    "m26-island-expanded-slow-ru": [islandSize, "scene=island&mode=expanded&state=thinking&slow=1&lang=ru&theme=dark"],
    "m26-island-expanded-reply-en": [islandSize, "scene=island&mode=expanded&state=idle&reply=1&lang=en&theme=dark"],
    "m26-island-expanded-reply-light": [islandSize, "scene=island&mode=expanded&state=idle&reply=1&lang=ru&theme=light"],
  "m26-island-expanded-reply-ru-dark": [islandSize, "scene=island&mode=expanded&state=idle&reply=1&lang=ru&theme=dark"],
  "m26-island-expanded-reply-en-light": [islandSize, "scene=island&mode=expanded&state=idle&reply=1&lang=en&theme=light"],
  "m26-island-expanded-error-ru-light": [islandSize, "scene=island&mode=expanded&state=error&lang=ru&theme=light"],
  "m26-island-expanded-slow-en-light": [islandSize, "scene=island&mode=expanded&state=thinking&slow=1&lang=en&theme=light"],
  "m26-island-tab-light": [{ w: 300, h: 100, scale: 2 }, "scene=island&mode=tab&theme=light"],
  "m26-island-compact-lyrics-light": [{ w: 480, h: 160, scale: 2 }, "scene=island&mode=compact&voiceOff=1&lyrics=1&theme=light"],
    "m26-island-expanded-error": [islandSize, "scene=island&mode=expanded&state=error&reply=1&replyText=Didn%27t+get+that&lang=en&theme=dark"],
    "m26f9-answer-hover": [islandSize, "scene=island&mode=expanded&state=idle&reply=1&lang=en&theme=dark&hoverBtn=insert"],
    "m26f9-answer-lang-menu": [{ w: 600, h: 560, scale: 2 }, "scene=island&mode=expanded&state=idle&reply=1&lang=ru&theme=dark&langMenu=1"],
    "m26f9-answer-copied": [islandSize, "scene=island&mode=expanded&state=idle&reply=1&lang=en&theme=dark&copied=1"],
  };
}
// T23: the settings window (Russian) in both colour schemes: pages, a search with results and the minimum window width.
function settingsShots() {
  const out = {};
  const pages = ["look", "phone", "voice", "reminders", "integrations", "actions", "settings", "about"];
  for (const scheme of ["light", "dark"]) {
    const base = `scene=settings&theme=studio&lang=ru&scheme=${scheme}`;
    for (const page of ["look", "voice", "reminders", "integrations"]) {
      const h = page === "voice" ? 1500 : page === "integrations" ? 900 : 760;
      out[`t23-${page}-${scheme}`] = [{ w: 1100, h, scale: 1 }, `${base}&go=${page}`];
    }
    out[`t23-search-${scheme}`] = [{ w: 1100, h: 760, scale: 1 }, `${base}&go=look&type=%D0%B3%D0%BE%D0%BB%D0%BE%D1%81`];
    // T23.R3: the Keys editor with the phone widgets in its preview.
    out[`t23-keys-${scheme}`] = [{ w: 1280, h: 900, scale: 1 }, `${base}&go=deck`];
    out[`t23-minwidth-${scheme}`] = [{ w: 820, h: 760, scale: 1 }, `${base}&go=settings`];
    out[`t23-minwidth-voice-${scheme}`] = [{ w: 820, h: 1500, scale: 1 }, `${base}&go=voice`];
    for (const w of [1280, 2560]) {
      for (const page of pages) {
        const h = page === "voice" ? 1400 : 900;
        out[`t23-${page}-${w}-${scheme}`] = [{ w, h, scale: 1 }, `${base}&go=${page}`];
      }
    }
  }
  return out;
}
const SHOTS = {
  "theme-amoled": [DECK, "scene=deck&theme=amoled"],
  "theme-studio": [DECK, "scene=deck&theme=studio"],
  "theme-material": [DECK, "scene=deck&theme=material"],
  "theme-glass": [DECK, "scene=deck&theme=glass"],
  settings: [{ w: 1100, h: 700, scale: 1 }, "scene=settings&theme=studio"],
  // Landing pictures (T22.3): `click` selects an item of the editor before the shot.
  "pc-editor": [{ w: 1600, h: 1000, scale: 1 }, "scene=settings&theme=studio", ".editor-item::6"],
  // M22 (T22.S): the new widgets, the windows and the pill labels.
  "m22-widgets": [DECK, "scene=deck&m22=1&theme=glass"],
  "m22-widgets-amoled": [DECK, "scene=deck&m22=1&theme=amoled"],
  "m22-reminders-window": [{ w: 1100, h: 700, scale: 1 }, "scene=settings&theme=studio&go=reminders"],
  "m22-lists-window": [{ w: 1100, h: 700, scale: 1 }, "scene=settings&theme=studio&go=reminders", ".segmented-track button::1"],
  "m22-voice-window": [{ w: 1100, h: 1300, scale: 1 }, "scene=settings&theme=studio&go=voice"],
  ...settingsShots(),
  ...t235bShots(),
  ...t24Shots(),
  ...m26Shots(),
  "phone-idle": [PHONE, "scene=phone&state=idle"],
  "phone-listening": [PHONE, "scene=phone&state=listening"],
  "phone-done": [PHONE, "scene=phone&state=done&heard=turn+the+volume+up&reply=Done"],
  "phone-confirm": [PHONE, "scene=phone&state=confirm&heard=lock+my+pc"],
};
const only = process.env.ONLY?.split(",");
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

async function shoot(name, [size, query, click]) {
  const profile = mkdtempSync(join(tmpdir(), "decka-voiceshots-"));
  const port = 9340 + Math.floor(Math.random() * 400);
  const chrome = spawn(CHROME, ["--headless=new", `--remote-debugging-port=${port}`, `--user-data-dir=${profile}`, "--lang=en-US", `--window-size=${size.w},${size.h}`, "about:blank"], { stdio: "ignore" });
  try {
    let target;
    for (let i = 0; i < 80 && !target; i++) {
      try {
        target = (await (await fetch(`http://127.0.0.1:${port}/json`)).json()).find((t) => t.type === "page");
      } catch {
        /* starting */
      }
      if (!target) await sleep(200);
    }
    if (!target) throw new Error(`${name}: Chrome CDP target did not appear`);
    const ws = new WebSocket(target.webSocketDebuggerUrl);
    await new Promise((r) => ws.addEventListener("open", r, { once: true }));
    let id = 0;
    const waiting = new Map();
    ws.addEventListener("message", (ev) => {
      const m = JSON.parse(String(ev.data));
      if (m.id && waiting.has(m.id)) waiting.get(m.id)(m);
      else if (m.method === "Runtime.consoleAPICalled" && process.env.DEBUG_SHOTS) console.log("console:", m.params.args.map((a) => a.value ?? a.description).join(" "));
      else if (m.method === "Runtime.exceptionThrown") console.error("page error:", m.params.exceptionDetails.exception?.description ?? m.params.exceptionDetails.text);
    });
    const send = (method, params = {}) => new Promise((resolve, reject) => {
      const mid = ++id;
      waiting.set(mid, (m) => (m.error ? reject(new Error(`${method}: ${m.error.message}`)) : resolve(m.result)));
      ws.send(JSON.stringify({ id: mid, method, params }));
    });
    await send("Page.enable");
    await send("Runtime.enable");
    await send("Emulation.setDeviceMetricsOverride", { width: size.w, height: size.h, deviceScaleFactor: size.scale, mobile: false });
    await send("Page.navigate", { url: `${base}?${query}` });
    for (let w = 0; w < 40; w++) {
      const r = await send("Runtime.evaluate", { expression: "document.getElementById('root')?.childElementCount ?? 0", returnByValue: true }).catch(() => null);
      if ((r?.result?.value ?? 0) > 0) break;
      await sleep(100);
    }
    await sleep(query.includes("theme=glass") ? 2200 : 1500);
    const scheme = new URLSearchParams(query).get("scheme");
    if (scheme) {
      await send("Emulation.setEmulatedMedia", { features: [{ name: "prefers-color-scheme", value: scheme }] });
      await sleep(300);
    }
    const typed = new URLSearchParams(query).get("type");
    if (typed) {
      // Type into the sidebar search: the results list opens under the field.
      await send("Runtime.evaluate", { expression: "document.querySelector('.sidebar-search-box input')?.focus()" });
      await send("Input.insertText", { text: typed });
      await sleep(500);
    }
    if (click) {
      const r = await send("Runtime.evaluate", { expression: `(() => { const [sel, txt] = ${JSON.stringify(click)}.split("::"); const e = [...document.querySelectorAll(sel)][Number(txt) || 0]; if (!e) return null; const b = e.getBoundingClientRect(); return JSON.stringify([b.x + b.width / 2, b.y + b.height / 2]); })()`, returnByValue: true });
      if (!r.result.value) throw new Error(`${name}: nothing matches ${click}`);
      const [x, y] = JSON.parse(r.result.value);
      await send("Input.dispatchMouseEvent", { type: "mouseMoved", x, y });
      await send("Input.dispatchMouseEvent", { type: "mousePressed", x, y, button: "left", clickCount: 1 });
      await send("Input.dispatchMouseEvent", { type: "mouseReleased", x, y, button: "left", clickCount: 1 });
      await sleep(600);
    }
    const focusSel = new URLSearchParams(query).get("focus");
    if (focusSel) {
      // Send Tab first so Chromium enters keyboard modality and renders :focus-visible rings.
      await send("Input.dispatchKeyEvent", { type: "keyDown", key: "Tab", code: "Tab", windowsVirtualKeyCode: 9 });
      await send("Input.dispatchKeyEvent", { type: "keyUp", key: "Tab", code: "Tab", windowsVirtualKeyCode: 9 });
      await send("Runtime.evaluate", {
        expression: `(() => { const e = document.querySelector(${JSON.stringify(focusSel)}); if (e) { e.scrollIntoView({ block: "center" }); e.focus({ focusVisible: true }); } return !!e; })()`,
        returnByValue: true,
      });
      await sleep(300);
    }
    const params = { format: "png" };
    if (query.includes("pad=1")) {
      // Crop to the pill with a 40 px margin on every side (T22.1).
      const r = await send("Runtime.evaluate", { expression: "(() => { const b = document.querySelector('.voice-pill-container').getBoundingClientRect(); return JSON.stringify([b.x, b.y, b.width, b.height]); })()", returnByValue: true });
      const [x, y, w, h] = JSON.parse(r.result.value);
      params.clip = { x: Math.floor(x) - 40, y: Math.floor(y) - 40, width: Math.ceil(w) + 80, height: Math.ceil(h) + 80, scale: 1 };
    }
    const shot = await send("Page.captureScreenshot", params);
    mkdirSync(outDir, { recursive: true });
    const file = join(outDir, `${name}.png`);
    writeFileSync(file, Buffer.from(shot.data, "base64"));
    ws.close();
    return file;
  } finally {
    chrome.kill();
  }
}

try {
  if (process.env.BENCH_ORB) {
    const profile = mkdtempSync(join(tmpdir(), "decka-orbbench-"));
    const port = 9390 + Math.floor(Math.random() * 200);
    const chrome = spawn(CHROME, ["--headless=new", `--remote-debugging-port=${port}`, `--user-data-dir=${profile}`, "--window-size=400,400", "about:blank"], { stdio: "ignore" });
    try {
      let target;
      for (let i = 0; i < 80 && !target; i++) {
        try { target = (await (await fetch(`http://127.0.0.1:${port}/json`)).json()).find((t) => t.type === "page"); } catch {}
        if (!target) await sleep(200);
      }
      const ws = new WebSocket(target.webSocketDebuggerUrl);
      await new Promise((r) => ws.addEventListener("open", r, { once: true }));
      let id = 0;
      const waiting = new Map();
      ws.addEventListener("message", (ev) => { const m = JSON.parse(String(ev.data)); if (m.id && waiting.has(m.id)) waiting.get(m.id)(m); });
      const send = (method, params = {}) => new Promise((resolve, reject) => {
        const mid = ++id;
        waiting.set(mid, (m) => (m.error ? reject(new Error(`${method}: ${m.error.message}`)) : resolve(m.result)));
        ws.send(JSON.stringify({ id: mid, method, params }));
      });
      await send("Page.enable");
      await send("Performance.enable");
      await send("Emulation.setDeviceMetricsOverride", { width: 400, height: 400, deviceScaleFactor: 1, mobile: false });
      await send("Page.navigate", { url: `${base}?scene=orb-bench&state=listening&size=120` });
      await sleep(1500);
      const m1 = await send("Performance.getMetrics");
      const t1 = performance.now();
      await sleep(4000);
      const m2 = await send("Performance.getMetrics");
      const t2 = performance.now();
      const metric = (res, k) => res.metrics.find((x) => x.name === k)?.value ?? 0;
      const taskMs = (metric(m2, "TaskDuration") - metric(m1, "TaskDuration")) * 1000;
      const scriptMs = (metric(m2, "ScriptDuration") - metric(m1, "ScriptDuration")) * 1000;
      const wallMs = t2 - t1;
      console.log(`ORB_BENCH size=120 state=listening wallMs=${wallMs.toFixed(1)} taskMs=${taskMs.toFixed(1)} scriptMs=${scriptMs.toFixed(1)} cpuPct=${((taskMs / wallMs) * 100).toFixed(2)}%`);
      ws.close();
    } finally {
      chrome.kill();
    }
  }
  for (const [name, spec] of Object.entries(SHOTS)) {
    if (only && !only.includes(name)) continue;
    console.log(await shoot(name, spec));
  }
} finally {
  server.close();
}
process.exit(0);
