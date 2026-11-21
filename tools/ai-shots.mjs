#!/usr/bin/env node
// Pictures of the AI limits widget on a phone-sized deck (800x360 css px, x3) for visual acceptance (T20.2).
// Builds tools/ai-shots with Vite, serves it, drives headless Chrome/Edge with the DevTools protocol.
//   node tools/ai-shots.mjs [outDir]       writes ai-2x1.png ai-2x2.png ai-4x2.png (AMOLED) and ai-2x2-material.png
import { spawn } from "node:child_process";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { createServer } from "node:http";
import { tmpdir } from "node:os";
import { extname, join, normalize } from "node:path";
import { createRequire } from "node:module";
import { pathToFileURL } from "node:url";

const root = new URL("..", import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, "$1");
const outDir = process.argv[2] ?? join(root, ".tmp", "shots");
const dist = join(root, ".tmp", "ai-shots-dist");
const CHROME = [process.env.CHROME_PATH, "C:/Program Files/Google/Chrome/Application/chrome.exe", "C:/Program Files (x86)/Google/Chrome/Application/chrome.exe", "C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe"].find((p) => p && existsSync(p));
if (!CHROME) throw new Error("Chrome or Edge not found: set CHROME_PATH");

const require = createRequire(join(root, "client", "package.json"));
const { build } = await import(pathToFileURL(require.resolve("vite")).href);
const react = (await import(pathToFileURL(require.resolve("@vitejs/plugin-react")).href)).default;
await build({ root: join(root, "tools", "ai-shots"), configFile: false, logLevel: "warn", plugins: [react()], base: "./", build: { outDir: dist, emptyOutDir: true, target: "chrome100" } });

const MIME = { ".html": "text/html", ".js": "text/javascript", ".css": "text/css", ".svg": "image/svg+xml", ".woff2": "font/woff2" };
const server = createServer((req, res) => {
  const path = normalize(decodeURIComponent((req.url ?? "/").split("?")[0])).replace(/^[\\/]+/, "") || "index.html";
  const file = join(dist, path);
  if (!file.startsWith(dist) || !existsSync(file)) return res.writeHead(404).end();
  res.writeHead(200, { "content-type": MIME[extname(file)] ?? "application/octet-stream" }).end(readFileSync(file));
});
await new Promise((r) => server.listen(0, "127.0.0.1", r));
const base = `http://127.0.0.1:${server.address().port}/index.html`;

const SHOTS = {
  "ai-2x1": "w=2&h=1&theme=amoled",
  "ai-2x2": "w=2&h=2&theme=amoled",
  "ai-4x2": "w=4&h=2&theme=amoled",
  "ai-2x2-material": "w=2&h=2&theme=material",
  "ai-2x1-ru": "w=2&h=1&theme=amoled&lang=ru",
  "ai-3x2": "w=3&h=2&theme=amoled",
  "ai-2x1-claude": "w=2&h=1&theme=amoled&show=claude",
  "ai-4x2-nodata": "w=4&h=2&theme=amoled&nodata=1",
};
const only = process.env.ONLY?.split(",");
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

async function shoot(name, query) {
  const profile = mkdtempSync(join(tmpdir(), "decka-aishots-"));
  const port = 9340 + Math.floor(Math.random() * 400);
  const chrome = spawn(CHROME, ["--headless=new", `--remote-debugging-port=${port}`, `--user-data-dir=${profile}`, "--window-size=800,360", "about:blank"], { stdio: "ignore" });
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
      else if (m.method === "Runtime.exceptionThrown") console.error("page error:", m.params.exceptionDetails.exception?.description ?? m.params.exceptionDetails.text);
    });
    const send = (method, params = {}) => new Promise((resolve, reject) => {
      const mid = ++id;
      waiting.set(mid, (m) => (m.error ? reject(new Error(`${method}: ${m.error.message}`)) : resolve(m.result)));
      ws.send(JSON.stringify({ id: mid, method, params }));
    });
    await send("Page.enable");
    await send("Runtime.enable");
    await send("Emulation.setDeviceMetricsOverride", { width: 800, height: 360, deviceScaleFactor: 3, mobile: false });
    await send("Page.navigate", { url: `${base}?${query}` });
    await sleep(1200);
    const box = (await send("Runtime.evaluate", { expression: `(() => { const b = document.querySelector(".tile").getBoundingClientRect(); return { x: b.x, y: b.y, width: b.width, height: b.height }; })()`, returnByValue: true })).result.value;
    const pad = 8;
    const shot = await send("Page.captureScreenshot", { format: "png", clip: { x: Math.max(0, box.x - pad), y: Math.max(0, box.y - pad), width: box.width + 2 * pad, height: box.height + 2 * pad, scale: 1 } });
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
  for (const [name, query] of Object.entries(SHOTS)) {
    if (only && !only.includes(name)) continue;
    console.log(await shoot(name, query));
  }
} finally {
  server.close();
}
process.exit(0);
