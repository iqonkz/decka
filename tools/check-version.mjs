// Every number of the version must agree (ARCHITECTURE.md 8a). Run by `npm test`; exit code 1 on a mismatch.
import { versionProblems, readVersions } from "./lib/version-files.mjs";

const root = new URL("..", import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, "$1");
const problems = versionProblems(root);
if (problems.length > 0) {
  console.error("check-version: the version numbers disagree:");
  for (const p of problems) console.error("  " + p);
  process.exit(1);
}
console.log(`check-version: OK (${readVersions(root)[0].value})`);
