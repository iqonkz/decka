// One version for the agent, the client and the APK (ARCHITECTURE.md 8a). The source of truth is the root package.json.
import { readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";

/** Every place that carries the number: `[path, label, pattern with the number in group 2]`. */
const PLACES = [
  ["package.json", "root package.json", /("version"\s*:\s*")([^"]+)(")/],
  ["shared/package.json", "shared/package.json", /("version"\s*:\s*")([^"]+)(")/],
  ["ui/package.json", "ui/package.json", /("version"\s*:\s*")([^"]+)(")/],
  ["client/package.json", "client/package.json", /("version"\s*:\s*")([^"]+)(")/],
  ["agent/package.json", "agent/package.json", /("version"\s*:\s*")([^"]+)(")/],
  ["agent/src-tauri/tauri.conf.json", "tauri.conf.json", /("version"\s*:\s*")([^"]+)(")/],
  ["Cargo.toml", "Cargo.toml [workspace.package]", /(\[workspace\.package\][^[]*?\nversion\s*=\s*")([^"]+)(")/],
  ["client/android/app/build.gradle", "build.gradle versionName", /(versionName\s+")([^"]+)(")/],
];
const VERSION_CODE = /(versionCode\s+)(\d+)()/;
const GRADLE = "client/android/app/build.gradle";

export const SEMVER = /^(\d+)\.(\d+)\.(\d+)$/;

/** `versionCode` of Android from `MAJOR.MINOR.PATCH`: MAJOR*10000 + MINOR*100 + PATCH. */
export function versionCode(version) {
  const m = SEMVER.exec(version);
  if (!m) throw new Error(`not a version: ${version}`);
  return Number(m[1]) * 10000 + Number(m[2]) * 100 + Number(m[3]);
}

/** The next version for `patch`, `minor` or `major`. */
export function nextVersion(version, kind) {
  const m = SEMVER.exec(version);
  if (!m) throw new Error(`not a version: ${version}`);
  const [major, minor, patch] = [Number(m[1]), Number(m[2]), Number(m[3])];
  if (kind === "major") return `${major + 1}.0.0`;
  if (kind === "minor") return `${major}.${minor + 1}.0`;
  if (kind === "patch") return `${major}.${minor}.${patch + 1}`;
  // An exact number (a release that is a bigger step than `minor`, e.g. 0.11.0 -> 0.20.0): it must be higher than now.
  const exact = SEMVER.exec(String(kind));
  const [a, b, c] = exact ? [Number(exact[1]), Number(exact[2]), Number(exact[3])] : [0, 0, 0];
  if (exact && String(kind) === `${a}.${b}.${c}`) {
    if (a > major || (a === major && (b > minor || (b === minor && c > patch)))) return String(kind);
    throw new Error(`${kind} is not higher than the current ${version}`);
  }
  throw new Error(`unknown kind: ${kind} (patch, minor, major or an exact X.Y.Z)`);
}

/** What every file says now: `[{ label, value }]`, with `versionCode` last (its value is the number as text). */
export function readVersions(root) {
  const out = PLACES.map(([path, label, pattern]) => {
    const m = pattern.exec(readFileSync(join(root, path), "utf8"));
    return { label, value: m ? m[2] : "(not found)" };
  });
  const code = VERSION_CODE.exec(readFileSync(join(root, GRADLE), "utf8"));
  out.push({ label: "build.gradle versionCode", value: code ? code[2] : "(not found)" });
  return out;
}

/** Writes `version` (and its `versionCode`) into every file. */
export function writeVersions(root, version) {
  for (const [path, , pattern] of PLACES) {
    const file = join(root, path);
    const text = readFileSync(file, "utf8");
    if (!pattern.test(text)) throw new Error(`no version in ${path}`);
    writeFileSync(file, text.replace(pattern, `$1${version}$3`));
  }
  const gradle = join(root, GRADLE);
  const text = readFileSync(gradle, "utf8");
  writeFileSync(gradle, text.replace(VERSION_CODE, `$1${versionCode(version)}`));
}

/** The problems of the current numbers (empty = all agree and `versionCode` follows the formula). */
export function versionProblems(root) {
  const found = readVersions(root);
  const root0 = found[0].value;
  const problems = [];
  if (!SEMVER.test(root0)) problems.push(`root package.json: "${root0}" is not MAJOR.MINOR.PATCH`);
  for (const { label, value } of found.slice(0, -1)) if (value !== root0) problems.push(`${label} is ${value}, root package.json is ${root0}`);
  const code = found.at(-1).value;
  if (SEMVER.test(root0) && code !== String(versionCode(root0))) problems.push(`build.gradle versionCode is ${code}, expected ${versionCode(root0)}`);
  return problems;
}

/** The text under `## Unreleased` (up to the next `## `), trimmed. */
export function unreleasedBody(changelog) {
  const m = /^## Unreleased[ \t]*\r?\n([\s\S]*?)(?=^## |(?![\s\S]))/m.exec(changelog);
  return m ? m[1].trim() : "";
}

/** Moves the Unreleased entries under `## <version> — <date>` and leaves an empty `## Unreleased` on top. */
export function releaseChangelog(changelog, version, date) {
  const body = unreleasedBody(changelog);
  if (body === "") throw new Error("CHANGELOG.md: ## Unreleased is empty");
  const eol = changelog.includes("\r\n") ? "\r\n" : "\n";
  const re = /^## Unreleased[ \t]*\r?\n[\s\S]*?(?=^## |(?![\s\S]))/m;
  return changelog.replace(re, `## Unreleased${eol}${eol}## ${version} — ${date}${eol}${eol}${body}${eol}${eol}`);
}
