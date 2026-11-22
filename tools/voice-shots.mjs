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
await build({ root: join(root, "tools", "voice-shots"), configFile: false, logLevel: "warn", plugins: [react()], base: "./", build: { outDir: dist, emptyOutDir: true, target: "chrome100" } });

const MIME = { ".html": "text/html", ".js": "text/javascript", ".css": "text/css", ".svg": "image/svg+xml", ".woff2": "font/woff2" };
const server = createServer((req, res) => {
  const path = normalize(decodeURIComponent((req.url ?? "/").split("?")[0])).replace(/^[\\/]+/, "") || "index.html";
  const file = join(dist, path);
  if (!file.startsWith(dist) || !existsSync(file)) return res.writeHead(404).end();
  res.writeHead(200, { "content-type": MIME[extname(file)] ?? "application/octet-stream" }).end(readFileSync(file));
});
await new Promise((r) => server.listen(0, "127.0.0.1", r));
const base = `http://127.0.0.1:${server.address().port}/index.html`;

const PILL = { w: 360, h: 160, scale: 3 };
const PHONE = { w: 800, h: 360, scale: 2 };
const DECK = { w: 1280, h: 600, scale: 1 };
const SHOTS = {
  "pill-listening": [PILL, "scene=pill&state=listening"],
  "pill-thinking": [PILL, "scene=pill&state=thinking&heard=open+calculator"],
  "pill-done": [PILL, "scene=pill&state=done&heard=open+calculator&reply=Opened+Calculator"],
  "pill-confirm": [PILL, "scene=pill&state=confirm&heard=lock+my+pc"],
  "pill-error": [PILL, "scene=pill&state=error&heard=blah&reply=Didn%27t+get+that"],
  voice: [PILL, "scene=pill&state=done&heard=open+calculator&reply=Opened+Calculator&pad=1"],
  "pill-done-ru": [PILL, "scene=pill&state=done&lang=ru&heard=%D0%BE%D1%82%D0%BA%D1%80%D0%BE%D0%B9+%D0%BA%D0%B0%D0%BB%D1%8C%D0%BA%D1%83%D0%BB%D1%8F%D1%82%D0%BE%D1%80&reply=Done"],
  "theme-amoled": [DECK, "scene=deck&theme=amoled"],
  "theme-studio": [DECK, "scene=deck&theme=studio"],
  "theme-material": [DECK, "scene=deck&theme=material"],
  "theme-glass": [DECK, "scene=deck&theme=glass"],
  settings: [{ w: 1100, h: 700, scale: 1 }, "scene=settings&theme=studio"],
  // Landing pictures (T22.3): `click` selects an item of the editor before the shot.
  "pc-editor": [{ w: 1600, h: 1000, scale: 1 }, "scene=settings&theme=studio", ".editor-item::6"],
  "pc-voice": [{ w: 1600, h: 900, scale: 1 }, "scene=pill&state=done&heard=open+calculator&reply=Opened+Calculator&desk=1"],
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
    for (let i = 0; i < 50 && !target; i++) {
      try {
        target = (await (await fetch(`http://127.0.0.1:${port}/json`)).json()).find((t) => t.type === "page");
      } catch {
        /* starting */
      }
      if (!target) await sleep(200);
    }
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
    await sleep(1200);
    if (click) {
      const r = await send("Runtime.evaluate", { expression: `(() => { const [sel, txt] = ${JSON.stringify(click)}.split("::"); const e = [...document.querySelectorAll(sel)][Number(txt) || 0]; if (!e) return null; const b = e.getBoundingClientRect(); return JSON.stringify([b.x + b.width / 2, b.y + b.height / 2]); })()`, returnByValue: true });
      if (!r.result.value) throw new Error(`${name}: nothing matches ${click}`);
      const [x, y] = JSON.parse(r.result.value);
      await send("Input.dispatchMouseEvent", { type: "mouseMoved", x, y });
      await send("Input.dispatchMouseEvent", { type: "mousePressed", x, y, button: "left", clickCount: 1 });
      await send("Input.dispatchMouseEvent", { type: "mouseReleased", x, y, button: "left", clickCount: 1 });
      await sleep(600);
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
  for (const [name, spec] of Object.entries(SHOTS)) {
    if (only && !only.includes(name)) continue;
    console.log(await shoot(name, spec));
  }
} finally {
  server.close();
}
process.exit(0);
