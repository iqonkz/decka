// Copies the SVG files of simple-icons (CC0) into agent/public/brands and writes brands.json ([title, slug, hex] rows)
// so the settings window can load the logos it shows on demand. The copy is not tracked by git.
import { cpSync, existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const candidates = [join(here, "..", "node_modules", "simple-icons"), join(here, "..", "..", "node_modules", "simple-icons")];
const pkg = candidates.find((p) => existsSync(join(p, "icons")));
const to = join(here, "..", "public", "brands");

if (!pkg) {
  console.error(`simple-icons not found in: ${candidates.join(", ")}`);
  process.exit(1);
}
mkdirSync(to, { recursive: true });
cpSync(join(pkg, "icons"), to, { recursive: true });
const data = JSON.parse(readFileSync(join(pkg, "data", "simple-icons.json"), "utf8"));
const rows = data.map((i) => [i.title, i.slug ?? i.title.toLowerCase().replace(/[^a-z0-9]/g, ""), i.hex]).sort((a, b) => a[0].localeCompare(b[0], "en", { sensitivity: "base" }));
writeFileSync(join(here, "..", "public", "brands.json"), JSON.stringify(rows));
console.log(`Brand logos copied to ${to} (${rows.length})`);
