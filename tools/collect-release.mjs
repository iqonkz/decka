// `npm run release:collect`: gathers the files for the GitHub release into release/ (git-ignored):
// Decka-Setup.exe (NSIS installer), Decka.apk (debug APK) and notes.md (this version's CHANGELOG section + install steps).
// Run after `npm run release:agent` and `npm run release:apk`; fails with a clear message if a build is missing.
import { copyFileSync, existsSync, mkdirSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { pathToFileURL } from "node:url";
import { readVersions } from "./lib/version-files.mjs";

export const INSTALL_BLOCK = `## Install

1. Windows: run Decka-Setup.exe. SmartScreen may warn because the installer is not code-signed: More info → Run anyway.
2. Android: open this page on the phone, download Decka.apk, allow installing from this source, open it.
3. Scan the QR code shown in the Decka window on the PC.
`;

/** The body of `## <version> ...` in the changelog, without the heading line. */
export function changelogSection(changelog, version) {
  const escaped = version.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const re = new RegExp(`^## ${escaped}(?![\\w.])[^\\r\\n]*\\r?\\n([\\s\\S]*?)(?=^## |(?![\\s\\S]))`, "m");
  const m = changelog.match(re);
  if (!m || m[1].trim() === "") throw new Error(`CHANGELOG.md has no "## ${version}" section; run version:bump first`);
  return m[1].trim();
}

export function collect(root) {
  const version = readVersions(root)[0].value;
  const nsisDir = join(root, "target", "release", "bundle", "nsis");
  const installer = existsSync(nsisDir) ? readdirSync(nsisDir).find((f) => f.startsWith(`Decka_${version}_`) && f.endsWith("-setup.exe")) : undefined;
  if (!installer) throw new Error(`no NSIS installer for ${version} in ${nsisDir}; run "npm run release:agent" first`);
  const apk = join(root, "client", "android", "app", "build", "outputs", "apk", "debug", "app-debug.apk");
  if (!existsSync(apk)) throw new Error(`no debug APK at ${apk}; run "npm run release:apk" first`);
  const notes = `${changelogSection(readFileSync(join(root, "CHANGELOG.md"), "utf8"), version)}\n\n${INSTALL_BLOCK}`;
  const out = join(root, "release");
  rmSync(out, { recursive: true, force: true });
  mkdirSync(out, { recursive: true });
  copyFileSync(join(nsisDir, installer), join(out, "Decka-Setup.exe"));
  copyFileSync(apk, join(out, "Decka.apk"));
  writeFileSync(join(out, "notes.md"), notes);
  return { version, out };
}

if (process.argv[1] && pathToFileURL(process.argv[1]).href === import.meta.url) {
  try {
    const root = new URL("..", import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, "$1");
    const { version, out } = collect(root);
    console.log(`release ${version}: ${out} (Decka-Setup.exe, Decka.apk, notes.md)`);
  } catch (e) {
    console.error(`release:collect: ${e.message}`);
    process.exit(1);
  }
}
