// `latest.json` of the one-click update (T20.3): the file `tauri-plugin-updater` reads from
// https://github.com/iqonkz/decka/releases/latest/download/latest.json. It is signed material only: the installer's minisign
// signature (the content of the `.sig` file Tauri writes next to the installer) and the URL of the installer.
export const REPO = "iqonkz/decka";

const STABLE = /^\d{1,9}\.\d{1,9}\.\d{1,9}$/;

/** URL of a release asset: the file name carries the version (`Decka-Setup-0.11.0.exe`). */
export const assetUrl = (version, name) => `https://github.com/${REPO}/releases/download/v${version}/${name}`;

/**
 * The manifest. `signature` is the text of the `.sig` file; `notes` is the changelog section; `pubDate` an RFC 3339 time.
 * Throws for a version that is not stable `X.Y.Z` or an empty signature: such a file would be refused by the plugin anyway.
 */
export function latestJson({ version, notes, signature, installerName, pubDate }) {
  if (!STABLE.test(version)) throw new Error(`latest.json: "${version}" is not a stable X.Y.Z version`);
  const sig = String(signature ?? "").trim();
  if (sig === "") throw new Error("latest.json: the installer signature is empty (was the installer built with the updater key?)");
  return {
    version,
    notes: String(notes ?? "").trim(),
    pub_date: pubDate,
    platforms: {
      "windows-x86_64": { signature: sig, url: assetUrl(version, installerName) },
    },
  };
}

/** Reads a manifest back (the same checks the agent relies on); `null` when it is not usable. */
export function parseLatest(text) {
  let v;
  try {
    v = JSON.parse(text);
  } catch {
    return null;
  }
  const p = v?.platforms?.["windows-x86_64"];
  if (typeof v?.version !== "string" || !STABLE.test(v.version)) return null;
  if (typeof p?.signature !== "string" || p.signature.trim() === "") return null;
  if (typeof p?.url !== "string" || !p.url.startsWith(`https://github.com/${REPO}/releases/download/`)) return null;
  return { version: v.version, notes: typeof v.notes === "string" ? v.notes : "", signature: p.signature, url: p.url };
}

/** `a > b` for stable `X.Y.Z` versions; anything else is not newer. */
export function isNewer(a, b) {
  if (!STABLE.test(a) || !STABLE.test(b)) return false;
  const x = a.split(".").map(Number);
  const y = b.split(".").map(Number);
  for (let i = 0; i < 3; i++) if (x[i] !== y[i]) return x[i] > y[i];
  return false;
}
