// Rasterizes docs/assets/decka-logo.svg and decka-glyph.svg into every PNG the project needs (DESIGN.md 7).
// Uses the installed Chrome in headless mode (no npm dependencies). Idempotent: the same input gives the same files.
//   node tools/render-logo.mjs          render into tools/logo-out/ and copy into the project
//   node tools/render-logo.mjs --no-copy   render only
import { execFileSync } from "node:child_process";
import { copyFileSync, existsSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const out = join(root, "tools", "logo-out");
const logo = pathToFileURL(join(root, "docs", "assets", "decka-logo.svg")).href;
const glyph = pathToFileURL(join(root, "docs", "assets", "decka-glyph.svg")).href;

const CHROME = [
  process.env.CHROME_PATH,
  "C:/Program Files/Google/Chrome/Application/chrome.exe",
  "C:/Program Files (x86)/Google/Chrome/Application/chrome.exe",
  "C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe",
  "C:/Program Files/Microsoft/Edge/Application/msedge.exe",
].find((p) => p && existsSync(p));
if (!CHROME) throw new Error("Chrome or Edge not found: set CHROME_PATH");

mkdirSync(out, { recursive: true });
const work = mkdtempSync(join(tmpdir(), "decka-logo-"));

/**
 * One PNG of `size` x `size` px with a transparent (or `bg`) background. `layers` are SVGs centered in it, `scale` is their share of the size.
 */
function render(name, size, layers, bg = "transparent") {
  const imgs = layers
    .map((l) => {
      const px = l.scale * size;
      const at = (size - px) / 2;
      return `<img src="${l.src}" style="position:absolute;left:${at}px;top:${at}px;width:${px}px;height:${px}px">`;
    })
    .join("");
  const html = join(work, `${name}.html`);
  writeFileSync(html, `<!doctype html><meta charset="utf-8"><body style="margin:0;overflow:hidden;background:${bg}"><div style="position:relative;width:${size}px;height:${size}px">${imgs}</div>`);
  const png = join(out, `${name}.png`);
  try {
    execFileSync(CHROME, [
      "--headless=new", "--disable-gpu", "--hide-scrollbars", "--force-device-scale-factor=1", "--no-first-run",
      `--user-data-dir=${join(work, "profile")}`,
      "--default-background-color=00000000", `--window-size=${size},${size}`, `--screenshot=${png}`, pathToFileURL(html).href,
    ], { stdio: "ignore", timeout: 60_000 });
  } catch (e) {
    if (!existsSync(png)) throw e;
  }
  return png;
}

const tile = (name, size) => render(name, size, [{ src: logo, scale: 1 }]);

try {
  // The tile in the sizes of the web client, the window and the Tauri icon source.
  for (const s of [16, 24, 32, 48, 64, 72, 96, 128, 192, 256, 512, 1024]) tile(`logo-${s}`, s);
  // Android adaptive foreground: the glyph is 45/108 wide (the SVG draws it 180 of 288 units wide, so the SVG is 2/3 of the layer).
  const FOREGROUND = 72 / 108;
  const density = { mdpi: 1, hdpi: 1.5, xhdpi: 2, xxhdpi: 3, xxxhdpi: 4 };
  for (const [d, k] of Object.entries(density)) {
    render(`android-foreground-${d}`, 108 * k, [{ src: glyph, scale: FOREGROUND }]);
    tile(`android-launcher-${d}`, 48 * k);
  }
  // Splash images: the sizes of the existing Capacitor splash files, the tile 96 dp (96 px at mdpi) in the middle.
  const splash = {
    "drawable": [480, 320, 1], "drawable-land-mdpi": [480, 320, 1], "drawable-land-hdpi": [800, 480, 1.5], "drawable-land-xhdpi": [1280, 720, 2],
    "drawable-land-xxhdpi": [1600, 960, 3], "drawable-land-xxxhdpi": [1920, 1280, 4],
    "drawable-port-mdpi": [320, 480, 1], "drawable-port-hdpi": [480, 800, 1.5], "drawable-port-xhdpi": [720, 1280, 2],
    "drawable-port-xxhdpi": [960, 1600, 3], "drawable-port-xxxhdpi": [1280, 1920, 4],
  };
  for (const [dir, [w, h, k]] of Object.entries(splash)) {
    const imgs = `<img src="${logo}" style="position:absolute;left:${(w - 96 * k) / 2}px;top:${(h - 96 * k) / 2}px;width:${96 * k}px;height:${96 * k}px">`;
    const html = join(work, `splash-${dir}.html`);
    writeFileSync(html, `<!doctype html><meta charset="utf-8"><body style="margin:0;overflow:hidden;background:#000"><div style="position:relative;width:${w}px;height:${h}px">${imgs}</div>`);
    const png = join(out, `splash-${dir}.png`);
    try {
      execFileSync(CHROME, ["--headless=new", "--disable-gpu", "--hide-scrollbars", "--force-device-scale-factor=1", "--no-first-run", `--user-data-dir=${join(work, "profile")}`, `--window-size=${w},${h}`, `--screenshot=${png}`, pathToFileURL(html).href], { stdio: "ignore", timeout: 60_000 });
    } catch (e) {
      if (!existsSync(png)) throw e;
    }
  }
} finally {
  rmSync(work, { recursive: true, force: true });
}

if (!process.argv.includes("--no-copy")) {
  const put = (from, to) => {
    const dest = join(root, to);
    mkdirSync(dirname(dest), { recursive: true });
    copyFileSync(join(out, from), dest);
  };
  const res = "client/android/app/src/main/res";
  const mip = { mdpi: "mdpi", hdpi: "hdpi", xhdpi: "xhdpi", xxhdpi: "xxhdpi", xxxhdpi: "xxxhdpi" };
  for (const d of Object.keys(mip)) {
    put(`android-foreground-${d}.png`, `${res}/mipmap-${d}/ic_launcher_foreground.png`);
    put(`android-launcher-${d}.png`, `${res}/mipmap-${d}/ic_launcher.png`);
    put(`android-launcher-${d}.png`, `${res}/mipmap-${d}/ic_launcher_round.png`);
  }
  for (const dir of [
    "drawable", "drawable-land-mdpi", "drawable-land-hdpi", "drawable-land-xhdpi", "drawable-land-xxhdpi", "drawable-land-xxxhdpi",
    "drawable-port-mdpi", "drawable-port-hdpi", "drawable-port-xhdpi", "drawable-port-xxhdpi", "drawable-port-xxxhdpi",
  ]) put(`splash-${dir}.png`, `${res}/${dir}/splash.png`);
  put("logo-32.png", "client/public/favicon.png");
  put("logo-192.png", "client/public/icon-192.png");
  put("logo-512.png", "client/public/icon-512.png");
  console.log("logo: rendered into tools/logo-out/ and copied into the project");
} else {
  console.log("logo: rendered into tools/logo-out/");
}
