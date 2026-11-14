import { DEFAULT_PORT, PAIR_ALPHABET } from "./protocol";

export interface PairUrl { code: string; hosts: string[]; port: number; agentName: string; }

const parsePort = (s: string): number | null => {
  if (!/^[0-9]{1,5}$/.test(s)) return null;
  const n = Number(s);
  return n >= 1 && n <= 65535 ? n : null;
};

// Uppercases, drops hyphens and spaces; returns null unless exactly 8 chars of PAIR_ALPHABET.
export function normalizeCode(input: string): string | null {
  const code = input.toUpperCase().replace(/[-\s]/g, "");
  if (code.length !== 8) return null;
  for (const ch of code) if (!PAIR_ALPHABET.includes(ch)) return null;
  return code;
}

// Parses the QR URL: http://<ip>:<port>/#c=<CODE>&h=<ip1>,<ip2>&p=<port>&n=<name>
export function parsePairUrl(input: string): PairUrl | null {
  let url: URL;
  try {
    url = new URL(input.trim());
  } catch {
    return null;
  }
  const params = new URLSearchParams(url.hash.replace(/^#/, ""));
  const code = normalizeCode(params.get("c") ?? "");
  if (!code) return null;

  let port = url.port ? parsePort(url.port) : DEFAULT_PORT;
  if (params.has("p")) port = parsePort(params.get("p") ?? "");
  if (port === null) return null;

  let hosts = (params.get("h") ?? "").split(",").map((h) => h.trim()).filter((h) => h !== "");
  if (hosts.length === 0) hosts = [url.hostname];
  return { code, hosts, port, agentName: params.get("n") ?? "" };
}

// "host" or "host:port"; port defaults to 9247.
export function parseHostInput(input: string): { host: string; port: number } | null {
  const s = input.trim();
  const m = /^([A-Za-z0-9.-]+)(?::([0-9]+))?$/.exec(s);
  if (!m || !m[1]) return null;
  if (m[2] === undefined) return { host: m[1], port: DEFAULT_PORT };
  const port = parsePort(m[2]);
  return port === null ? null : { host: m[1], port };
}
