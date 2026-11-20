// Tests of the version tools on temporary copies of the files (never on the repository itself).
import { test } from "node:test";
import assert from "node:assert/strict";
import { execFileSync, spawnSync } from "node:child_process";
import { cpSync, existsSync, mkdirSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { bump } from "./bump-version.mjs";
import { nextVersion, readVersions, releaseChangelog, unreleasedBody, versionCode, versionProblems, writeVersions } from "./lib/version-files.mjs";

const repo = join(dirname(fileURLToPath(import.meta.url)), "..");
const FILES = ["package.json", "shared/package.json", "ui/package.json", "client/package.json", "agent/package.json", "agent/src-tauri/tauri.conf.json", "Cargo.toml", "client/android/app/build.gradle"];
const CHANGELOG = "# Changelog\n\n## Unreleased\n\n### Added\n- A new thing.\n\n## 0.1.0\n\n- First release.\n";

/** A temporary copy of the version files with a CHANGELOG. */
function copy(changelog = CHANGELOG) {
  const root = mkdtempSync(join(tmpdir(), "decka-version-"));
  for (const f of FILES) {
    mkdirSync(dirname(join(root, f)), { recursive: true });
    cpSync(join(repo, f), join(root, f));
  }
  writeVersions(root, "0.1.0"); // the tests assume a 0.1.0 baseline whatever version the repository is at now
  writeFileSync(join(root, "CHANGELOG.md"), changelog);
  return root;
}

test("the repository's own numbers agree", () => {
  assert.deepEqual(versionProblems(repo), []);
});

test("versionCode follows MAJOR*10000 + MINOR*100 + PATCH", () => {
  assert.equal(versionCode("0.1.0"), 100);
  assert.equal(versionCode("1.2.3"), 10203);
  assert.equal(versionCode("0.10.9"), 1009);
});

test("patch, minor and major", () => {
  assert.equal(nextVersion("0.1.0", "patch"), "0.1.1");
  assert.equal(nextVersion("0.1.9", "minor"), "0.2.0");
  assert.equal(nextVersion("1.4.7", "major"), "2.0.0");
  assert.throws(() => nextVersion("1.0.0", "huge"));
});

test("a bump changes every file and versionCode, and the changelog moves under the new version with the date", () => {
  const root = copy();
  assert.equal(bump(root, "minor", "2026-11-16"), "0.2.0");
  for (const { label, value } of readVersions(root)) assert.equal(value, label.includes("versionCode") ? "200" : "0.2.0", label);
  assert.deepEqual(versionProblems(root), []);
  const text = readFileSync(join(root, "CHANGELOG.md"), "utf8");
  assert.match(text, /## Unreleased\n\n## 0\.2\.0 — 2026-11-16\n\n### Added\n- A new thing\.\n\n## 0\.1\.0/);
  assert.equal(unreleasedBody(text), "");
});

test("an empty Unreleased stops the bump and changes nothing", () => {
  const root = copy("# Changelog\n\n## Unreleased\n\n## 0.1.0\n\n- First release.\n");
  assert.throws(() => bump(root, "patch", "2026-11-16"), /empty/);
  assert.deepEqual(versionProblems(root), []);
  assert.equal(readVersions(root)[0].value, "0.1.0");
});

test("numbers that disagree are found", () => {
  const root = copy();
  const file = join(root, "ui/package.json");
  writeFileSync(file, readFileSync(file, "utf8").replace(/"version": "[^"]+"/, '"version": "0.9.0"'));
  assert.equal(versionProblems(root).length, 1);
  assert.throws(() => bump(root, "patch", "2026-11-16"), /disagree/);
});

test("releaseChangelog keeps older sections", () => {
  const out = releaseChangelog(CHANGELOG, "0.1.1", "2026-12-01");
  assert.ok(out.indexOf("## 0.1.1") < out.indexOf("## 0.1.0"));
});

test("the script refuses a dirty working tree and works on a clean one (tag included)", () => {
  const root = copy();
  const git = (...args) => execFileSync("git", ["-c", "user.name=t", "-c", "user.email=t@example.com", "-c", "core.autocrlf=false", ...args], { cwd: root, encoding: "utf8" });
  git("init", "-q");
  git("add", "-A");
  git("commit", "-q", "-m", "start");
  const run = () => spawnSync(process.execPath, [join(repo, "tools/bump-version.mjs"), "patch"], { env: { ...process.env, DECKA_BUMP_ROOT: root, GIT_AUTHOR_NAME: "t", GIT_AUTHOR_EMAIL: "t@example.com", GIT_COMMITTER_NAME: "t", GIT_COMMITTER_EMAIL: "t@example.com" }, encoding: "utf8" });

  writeFileSync(join(root, "stray.txt"), "dirty");
  const refused = run();
  assert.equal(refused.status, 1);
  assert.match(refused.stderr, /not clean/);
  assert.equal(readVersions(root)[0].value, "0.1.0");

  git("add", "-A");
  git("commit", "-q", "-m", "stray");
  const done = run();
  assert.equal(done.status, 0, done.stderr);
  assert.equal(readVersions(root)[0].value, "0.1.1");
  assert.match(git("tag"), /v0\.1\.1/);
  assert.ok(existsSync(join(root, "CHANGELOG.md")));
});

test("SHA256SUMS.txt has one `<hash>  <name>` line per release file", async () => {
  const { sumsText } = await import("./collect-release.mjs");
  const dir = mkdtempSync(join(tmpdir(), "decka-sums-"));
  writeFileSync(join(dir, "a.bin"), "abc");
  writeFileSync(join(dir, "b.bin"), "");
  assert.equal(
    sumsText(dir, ["a.bin", "b.bin"]),
    "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad  a.bin\ne3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855  b.bin\n",
  );
});
