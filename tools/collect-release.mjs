// `npm run release:collect`: gathers the files for the GitHub release into release/ (git-ignored):
// Decka-Setup-<version>.exe (NSIS installer), Decka-<version>.apk (signed release APK), latest.json (one-click update), SHA256SUMS.txt
// with its signature SHA256SUMS.txt.sig (updater key) and notes.md (this version's CHANGELOG section + install steps).
// Run after `npm run release:agent` and `npm run release:apk`; fails with a clear message if a build is missing.
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { copyFileSync, existsSync, mkdirSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { pathToFileURL } from "node:url";
import { latestJson } from "./lib/latest-json.mjs";
import { readVersions } from "./lib/version-files.mjs";

/** The file names of a release carry the version, so a downloaded file says which one it is. */
export const installerName = (version) => `Decka-Setup-${version}.exe`;
export const apkName = (version) => `Decka-${version}.apk`;

export const installBlock = (version) => `## Install

1. Windows: run ${installerName(version)}. SmartScreen may warn because the installer is not code-signed: More info → Run anyway.
2. Android: open this page on the phone, download ${apkName(version)}, allow installing from this source, open it. If you have an older Decka (debug-signed) installed, uninstall it first: the APK is signed with a new key, so it cannot be installed over the old one. Then pair again.
3. Scan the QR code shown in the Decka window on the PC.

To check a download, compare \`Get-FileHash .\\${installerName(version)}\` with the line in \`SHA256SUMS.txt\`.
`;

/** `SHA256SUMS.txt`: one `<sha256 hex>  <file name>` line per file, so that `Get-FileHash` output can be compared by eye (README: Verify the download). */
export function sumsText(dir, names) {
  return names.map((name) => `${createHash("sha256").update(readFileSync(join(dir, name))).digest("hex")}  ${name}`).join("\n") + "\n";
}

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
  const apk = join(root, "client", "android", "app", "build", "outputs", "apk", "release", "app-release.apk");
  if (!existsSync(apk)) throw new Error(`no signed release APK at ${apk}; run "npm run release:apk" first`);
  const sigFile = join(nsisDir, `${installer}.sig`);
  if (!existsSync(sigFile)) throw new Error(`no update signature ${sigFile}; "npm run release:agent" builds it with the updater key (T20.3)`);
  const changes = changelogSection(readFileSync(join(root, "CHANGELOG.md"), "utf8"), version);
  const notes = `${changes}\n\n${installBlock(version)}`;
  const out = join(root, "release");
  rmSync(out, { recursive: true, force: true });
  mkdirSync(out, { recursive: true });
  copyFileSync(join(nsisDir, installer), join(out, installerName(version)));
  copyFileSync(apk, join(out, apkName(version)));
  writeFileSync(join(out, "notes.md"), notes);
  writeFileSync(join(out, "SHA256SUMS.txt"), sumsText(out, [installerName(version), apkName(version)]));
  // The one-click update of the agent reads latest.json; the phone's update checks SHA256SUMS.txt against its signature.
  const manifest = latestJson({ version, notes: changes, signature: readFileSync(sigFile, "utf8"), installerName: installerName(version), pubDate: new Date().toISOString().replace(/\.\d{3}Z$/, "Z") });
  writeFileSync(join(out, "latest.json"), `${JSON.stringify(manifest, null, 2)}\n`);
  signFile(root, join(out, "SHA256SUMS.txt"));
  return { version, out };
}

/** Signs `file` with the updater key (TAURI_SIGNING_PRIVATE_KEY and its password come from the process environment) -> `<file>.sig`. */
export function signFile(root, file) {
  if (!process.env.TAURI_SIGNING_PRIVATE_KEY || !process.env.TAURI_SIGNING_PRIVATE_KEY_PASSWORD) throw new Error("the updater key is not in the environment; run this through `npm run release:collect`");
  const run = spawnSync("npm", ["run", "--silent", "tauri", "--", "signer", "sign", file], { cwd: root, shell: true, stdio: ["ignore", "ignore", "pipe"], encoding: "utf8" });
  if (run.status !== 0 || !existsSync(`${file}.sig`)) throw new Error(`signing ${file} failed${run.stderr ? `: ${run.stderr.trim().split("\n").pop()}` : ""}`);
}

if (process.argv[1] && pathToFileURL(process.argv[1]).href === import.meta.url) {
  try {
    const root = new URL("..", import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, "$1");
    const { version, out } = collect(root);
    console.log(`release ${version}: ${out} (${installerName(version)}, ${apkName(version)}, latest.json, SHA256SUMS.txt, SHA256SUMS.txt.sig, notes.md)`);
  } catch (e) {
    console.error(`release:collect: ${e.message}`);
    process.exit(1);
  }
}
