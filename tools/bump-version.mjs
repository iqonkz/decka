// `npm run version:bump -- patch|minor|major` (ARCHITECTURE.md 8a): changes every version number, moves `## Unreleased` of
// CHANGELOG.md under the new version with today's date, commits `release: vX.Y.Z` and tags `vX.Y.Z`. Does not push.
// Refuses to run on a dirty working tree or with an empty `## Unreleased`.
import { execFileSync } from "node:child_process";
import { readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { pathToFileURL } from "node:url";
import { nextVersion, readVersions, releaseChangelog, unreleasedBody, versionProblems, writeVersions } from "./lib/version-files.mjs";

/** The whole bump without git (the tests run it on a copy of the files). Returns the new version. */
export function bump(root, kind, date) {
  const problems = versionProblems(root);
  if (problems.length > 0) throw new Error(`the numbers disagree: ${problems.join("; ")}`);
  const changelogPath = join(root, "CHANGELOG.md");
  const changelog = readFileSync(changelogPath, "utf8");
  if (unreleasedBody(changelog) === "") throw new Error("CHANGELOG.md: ## Unreleased is empty");
  const version = nextVersion(readVersions(root)[0].value, kind);
  const released = releaseChangelog(changelog, version, date);
  writeVersions(root, version);
  writeFileSync(changelogPath, released);
  return version;
}

if (process.argv[1] && pathToFileURL(process.argv[1]).href === import.meta.url) {
  // `DECKA_BUMP_ROOT` points the script at a copy (the tests); normally it is the repository it lives in.
  const root = process.env.DECKA_BUMP_ROOT ?? new URL("..", import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, "$1");
  const kind = process.argv[2];
  try {
    const dirty = execFileSync("git", ["status", "--porcelain"], { cwd: root, encoding: "utf8" }).trim();
    if (dirty !== "") throw new Error("the working tree is not clean; commit or stash first");
    const version = bump(root, kind, new Date().toISOString().slice(0, 10));
    execFileSync("git", ["add", "-A"], { cwd: root });
    execFileSync("git", ["commit", "-m", `release: v${version}`], { cwd: root });
    execFileSync("git", ["tag", "-a", `v${version}`, "-m", `v${version}`], { cwd: root });
    console.log(`version ${version}: committed and tagged; push with git push --follow-tags`);
  } catch (e) {
    console.error(`version:bump: ${e.message}`);
    process.exit(1);
  }
}
