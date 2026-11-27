// Finds visible strings that are not wrapped in t() (DESIGN.md 6): JSX text nodes and the attributes aria-label,
// title, placeholder and alt with Latin letters. Run by `npm test`; the exit code is the number of findings (0 = clean).
import { readdirSync, readFileSync, statSync } from "node:fs";
import { join, relative } from "node:path";

const ROOT = new URL("..", import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, "$1");
const DIRS = ["agent/src", "client/src", "ui/src"];

/** Words that are the same in every language (product, technology and unit names). Each entry: why. */
const WORDS = new Set([
  "USB", // technology name, DESIGN.md 6
  "Wi-Fi", // technology name
  "QR", // technology name
  "Decka", // product name
  "adb", // the Android tool, a program name
  "PowerShell", // program name
  "CPU", // hardware term shown on the sys widget the same in both languages
  "RAM", // hardware term, same as CPU
  "OK", // short universal word inside the sample widgets
  "X", "Y", "W", "H", // coordinates of the position fields
  "AMOLED", "Studio", "Material", "Glass", // names of the four looks are product names
  "Phosphor", // the icon library
  "English", // a language is named in that language (DESIGN.md 6)
]);

/** Attribute values that are examples of data, not words of the interface. */
const ATTRIBUTE_EXAMPLES = new Set([
  "XXXX-XXXX", // pairing code mask
  "192.168.1.10", // address example
  "github.com", // address example
  "https://example.com/hook", // webhook address example
  "Content-Type: application/json", // webhook header example
  "code.exe", // program example
  "audio.muted", // variable example
  "true", // value example
  "200", // status example
  "curl.exe (JSON)", // a program name with its input format
  "curl.exe", // a program name
  "light.bedroom", // an entity id of Home Assistant
  "http://homeassistant.local:8123", // address example
]);

function* files(dir) {
  for (const name of readdirSync(dir)) {
    const path = join(dir, name);
    if (statSync(path).isDirectory()) yield* files(path);
    else if (path.endsWith(".tsx")) yield path;
  }
}

const hasWords = (text) => (text.match(/[A-Za-z]{2,}/g) ?? []).some((w) => !WORDS.has(w));

const findings = [];
for (const dir of DIRS) {
  for (const file of files(join(ROOT, dir))) {
    const source = readFileSync(file, "utf8");
    const lineOf = (index) => source.slice(0, index).split("\n").length;
    const report = (index, text) => findings.push(`${relative(ROOT, file)}:${lineOf(index)}  ${text.trim().replace(/\s+/g, " ").slice(0, 80)}`);

    // JSX text: between `>` and `<`, plain prose only (code between a generic `>` and the next `<` has `(`, `=`, `;`...).
    for (const m of source.matchAll(/>([^<>{}]+)</g)) {
      const text = m[1];
      if (!/^[\sA-Za-z0-9,.'’!?:…—·&;%/+×-]+$/.test(text) || !hasWords(text)) continue;
      // Code that sits between a `>` and a `<` (`: x ? <a/> : <b/>`, `; return <p>`) is not text.
      if (/^\s*[;:?]/.test(text) || /\breturn\b/.test(text) || /[A-Za-z]\.[a-z]/.test(text) || /[A-Za-z] \?\s*$/.test(text)) continue;
      // `=>` of an arrow function and `->` in comments are not tags.
      if (source[m.index - 1] === "=" || source[m.index - 1] === "-") continue;
      report(m.index, text);
    }

    // Attributes with a literal string.
    for (const m of source.matchAll(/\b(aria-label|title|placeholder|alt)="([^"]*)"/g)) {
      const value = m[2];
      if (ATTRIBUTE_EXAMPLES.has(value) || !hasWords(value)) continue;
      report(m.index, `${m[1]}="${value}"`);
    }
  }
}

if (findings.length > 0) {
  console.error(`check-i18n: ${findings.length} visible string(s) not wrapped in t():`);
  for (const f of findings) console.error("  " + f);
  process.exit(1);
}
console.log("check-i18n: OK");
