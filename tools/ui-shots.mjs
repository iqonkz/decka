#!/usr/bin/env node
// Screenshots of the settings window for visual acceptance (T17.8, T19.4). No dependencies: the built window (agent/dist) is
// served by a tiny static server, the Tauri IPC is replaced by a mock, headless Chrome is driven over the DevTools protocol
// (Node >= 22 has a global WebSocket). Run `npm run build:ui` first.
//   node tools/ui-shots.mjs <scenario|all> [outDir]      scenarios: ui-library-ru ui-sets-ru ui-apps-ru i18n-de i18n-ar i18n-kk i18n-ja
import { spawn } from "node:child_process";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { createServer } from "node:http";
import { tmpdir } from "node:os";
import { extname, join, normalize } from "node:path";

const root = new URL("..", import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, "$1");
const dist = join(root, "agent", "dist");
const CHROME = [process.env.CHROME_PATH, "C:/Program Files/Google/Chrome/Application/chrome.exe", "C:/Program Files (x86)/Google/Chrome/Application/chrome.exe", "C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe"].find((p) => p && existsSync(p));
if (!CHROME) throw new Error("Chrome or Edge not found: set CHROME_PATH");

const layoutBase = JSON.parse(readFileSync(join(root, "shared", "fixtures", "layout.default.json"), "utf8"));
const actions = JSON.parse(readFileSync(join(root, "shared", "fixtures", "actions.default.json"), "utf8"));
const APPS = ["Visual Studio Code", "Google Chrome", "Spotify", "Microsoft Word", "Discord", "OBS Studio", "Telegram Desktop", "Steam"].map((name) => ({ name, program: `C:\\Apps\\${name}.exe`, args: [] }));

/** The init script: a Tauri IPC with just enough answers for the window to draw itself. */
function mockScript(language) {
  const layout = { ...layoutBase, display: { ...layoutBase.display, language } };
  const config = { version: 1, agentName: "DESKTOP", port: 9247, bind: "0.0.0.0", apiToken: "dk_x", autostart: false, usb: { enabled: false, adbPath: "adb" }, collectors: { sys: { enabled: true, intervalSec: 2 }, checks: [] } };
  const status = { version: "0.10.0", listening: { bind: "0.0.0.0", port: 9247 }, lanIps: ["192.168.1.5"], clients: [], usb: { enabled: false, adbFound: false, devices: [] }, focusApp: "code.exe", configDir: "C:\\Users\\me\\AppData\\Roaming\\Decka" };
  return `(() => {
    const answers = ${JSON.stringify({ get_layout: layout, get_actions: actions, get_config: config, get_status: status, is_first_run: false, list_installed_apps: APPS, list_devices: [], get_update_status: { available: null, checkedAt: null, failed: false } })};
    window.__TAURI_INTERNALS__ = {
      invoke: async (cmd) => (cmd in answers ? JSON.parse(JSON.stringify(answers[cmd])) : []),
      transformCallback: () => 1, unregisterCallback() {}, convertFileSrc: (p) => p, metadata: { currentWindow: { label: "main" }, currentWebview: { label: "main" } },
    };
    window.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener() {} };
    try { localStorage.setItem("decka.editor.tips.v1", "1"); } catch {}
  })();`;
}

const MIME = { ".html": "text/html", ".js": "text/javascript", ".css": "text/css", ".svg": "image/svg+xml", ".png": "image/png", ".json": "application/json", ".woff2": "font/woff2" };
function serve() {
  const server = createServer((req, res) => {
    const path = normalize(decodeURIComponent((req.url ?? "/").split("?")[0])).replace(/^[\\/]+/, "") || "index.html";
    const file = join(dist, path);
    if (!file.startsWith(dist) || !existsSync(file)) return res.writeHead(404).end();
    res.writeHead(200, { "content-type": MIME[extname(file)] ?? "application/octet-stream" }).end(readFileSync(file));
  });
  return new Promise((resolve) => server.listen(0, "127.0.0.1", () => resolve(server)));
}

/** One CDP session over the page's WebSocket. */
async function connect(port) {
  let target;
  for (let i = 0; i < 50 && !target; i++) {
    try {
      target = (await (await fetch(`http://127.0.0.1:${port}/json`)).json()).find((t) => t.type === "page");
    } catch {
      /* Chrome is starting */
    }
    if (!target) await new Promise((r) => setTimeout(r, 200));
  }
  if (!target) throw new Error("Chrome did not start");
  const ws = new WebSocket(target.webSocketDebuggerUrl);
  await new Promise((r) => ws.addEventListener("open", r, { once: true }));
  let id = 0;
  const waiting = new Map();
  ws.addEventListener("message", (ev) => {
    const m = JSON.parse(String(ev.data));
    if (m.id && waiting.has(m.id)) waiting.get(m.id)(m);
    else if (m.method === "Runtime.exceptionThrown") console.error("page error:", m.params.exceptionDetails.exception?.description ?? m.params.exceptionDetails.text);
  });
  const send = (method, params = {}) => new Promise((resolve, reject) => {
    const mid = ++id;
    waiting.set(mid, (m) => (m.error ? reject(new Error(`${method}: ${m.error.message}`)) : resolve(m.result)));
    ws.send(JSON.stringify({ id: mid, method, params }));
  });
  return { send, close: () => ws.close() };
}

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

/** Scenarios: language, window size and what to click before the picture. `eval` runs in the page; strings are matched against button texts. */
const SCENARIOS = {
  "ui-library-ru": { language: "ru", width: 1000, height: 760, steps: [{ hover: ".library .palette-item" }] },
  "ui-sets-ru": { language: "ru", width: 1000, height: 760, steps: [{ click: ".segments .segment:nth-child(2)" }] },
  "ui-apps-ru": { language: "ru", width: 1000, height: 760, steps: [{ click: ".segments .segment:nth-child(3)" }, { wait: 400 }] },
  "ui-library-ru-min": { language: "ru", width: 760, height: 700, steps: [{ hover: ".library .palette-item" }] },
  "i18n-de": { language: "de", width: 1000, height: 760, steps: [{ hover: ".library .palette-item" }] },
  "i18n-de-more": { language: "de", width: 1000, height: 760, steps: [{ click: ".tabs .tab:nth-child(4)" }, { wait: 300 }] },
  "i18n-ar": { language: "ar", width: 1000, height: 760, steps: [{ hover: ".library .palette-item" }] },
  "i18n-ar-more": { language: "ar", width: 1000, height: 760, steps: [{ click: ".tabs .tab:nth-child(4)" }, { wait: 300 }] },
  "ui-library-kk": { language: "kk", width: 1000, height: 760, steps: [{ hover: ".library .palette-item" }] },
  "i18n-kk": { language: "kk", width: 1000, height: 760, steps: [{ hover: ".library .palette-item" }] },
  "i18n-ja": { language: "ja", width: 1000, height: 760, steps: [{ hover: ".library .palette-item" }] },
};

async function shoot(name, scenario, base, outDir) {
  const profile = mkdtempSync(join(tmpdir(), "decka-shots-"));
  const port = 9340 + Math.floor(Math.random() * 400);
  const chrome = spawn(CHROME, ["--headless=new", `--remote-debugging-port=${port}`, `--user-data-dir=${profile}`, `--window-size=${scenario.width},${scenario.height}`, "--hide-scrollbars=false", "--force-device-scale-factor=1", "about:blank"], { stdio: "ignore" });
  try {
    const cdp = await connect(port);
    await cdp.send("Page.enable");
    await cdp.send("Runtime.enable");
    await cdp.send("Emulation.setDeviceMetricsOverride", { width: scenario.width, height: scenario.height, deviceScaleFactor: 1, mobile: false });
    await cdp.send("Page.addScriptToEvaluateOnNewDocument", { source: mockScript(scenario.language) });
    await cdp.send("Page.navigate", { url: base });
    await sleep(1500);
    const rect = async (selector) => {
      const r = await cdp.send("Runtime.evaluate", { expression: `(() => { const e = document.querySelector(${JSON.stringify(selector)}); if (!e) return null; const b = e.getBoundingClientRect(); return { x: b.x + b.width / 2, y: b.y + b.height / 2 }; })()`, returnByValue: true });
      return r.result.value;
    };
    for (const step of scenario.steps) {
      if (step.wait) await sleep(step.wait);
      if (step.click || step.hover) {
        const at = await rect(step.click ?? step.hover);
        if (!at) {
          const dump = await cdp.send("Runtime.evaluate", { expression: "document.body.innerText.slice(0, 400)", returnByValue: true });
          mkdirSync(outDir, { recursive: true });
          writeFileSync(join(outDir, `${name}-debug.png`), Buffer.from((await cdp.send("Page.captureScreenshot", { format: "png" })).data, "base64"));
          throw new Error(`${name}: nothing matches ${step.click ?? step.hover}; the page says: ${dump.result.value}`);
        }
        await cdp.send("Input.dispatchMouseEvent", { type: "mouseMoved", x: at.x, y: at.y });
        if (step.click) {
          await cdp.send("Input.dispatchMouseEvent", { type: "mousePressed", x: at.x, y: at.y, button: "left", clickCount: 1 });
          await cdp.send("Input.dispatchMouseEvent", { type: "mouseReleased", x: at.x, y: at.y, button: "left", clickCount: 1 });
        }
        await sleep(350);
      }
    }
    const shot = await cdp.send("Page.captureScreenshot", { format: "png" });
    mkdirSync(outDir, { recursive: true });
    const file = join(outDir, `${name}.png`);
    writeFileSync(file, Buffer.from(shot.data, "base64"));
    cdp.close();
    return file;
  } finally {
    chrome.kill();
  }
}

const [which = "all", outDir = join(root, ".tmp", "shots")] = process.argv.slice(2);
const server = await serve();
const base = `http://127.0.0.1:${server.address().port}/index.html`;
try {
  for (const [name, scenario] of Object.entries(SCENARIOS)) {
    if (which !== "all" && which !== name) continue;
    console.log(await shoot(name, scenario, base, outDir));
  }
} finally {
  server.close();
}
process.exit(0);
