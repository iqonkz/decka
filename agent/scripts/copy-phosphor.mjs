// Copies the SVG files of @phosphor-icons/core into agent/public/phosphor so the settings window can load the ones it
// shows on demand (a static file per icon instead of thousands of JavaScript chunks). The copy is not tracked by git.
import { cpSync, existsSync, mkdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
// npm hoists the package to the repository root; a local install (agent/node_modules) is checked first.
const candidates = [join(here, "..", "node_modules", "@phosphor-icons", "core", "assets"), join(here, "..", "..", "node_modules", "@phosphor-icons", "core", "assets")];
const from = candidates.find((p) => existsSync(p));
const to = join(here, "..", "public", "phosphor");

if (!from) {
  console.error(`Phosphor assets not found in: ${candidates.join(", ")}`);
  process.exit(1);
}
mkdirSync(to, { recursive: true });
cpSync(from, to, { recursive: true });
console.log(`Phosphor icons copied to ${to}`);
