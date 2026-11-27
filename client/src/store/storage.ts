// The paired PCs of this phone (T32.4): a list with the current one. The tokens stay where the single pairing was kept
// before (the app's private storage).
const KEY = "decka.pcs.v1";
/** The single pairing of earlier versions: read once, then replaced by the list. */
const OLD_KEY = "decka.conn.v1";

export interface SavedConn {
  token: string;
  deviceId: string;
  hosts: string[];
  port: number;
  agentName: string;
}

export interface SavedPcs {
  /** `deviceId` of the PC the phone connects to; null only when the list is empty. */
  current: string | null;
  pcs: SavedConn[];
}

function parseConn(v: unknown): SavedConn | null {
  if (typeof v !== "object" || v === null) return null;
  const c = v as Partial<SavedConn>;
  if (
    typeof c.token !== "string" ||
    typeof c.deviceId !== "string" ||
    !Array.isArray(c.hosts) ||
    typeof c.port !== "number" ||
    typeof c.agentName !== "string"
  ) {
    return null;
  }
  return { token: c.token, deviceId: c.deviceId, hosts: c.hosts.map(String), port: c.port, agentName: c.agentName };
}

function read(key: string): unknown {
  try {
    const raw = localStorage.getItem(key);
    return raw ? (JSON.parse(raw) as unknown) : null;
  } catch {
    return null; // damaged or no storage: as if nothing was saved
  }
}

function write(next: SavedPcs): void {
  try {
    if (next.pcs.length === 0) localStorage.removeItem(KEY);
    else localStorage.setItem(KEY, JSON.stringify(next));
    localStorage.removeItem(OLD_KEY);
  } catch {
    // Storage unavailable (private mode, quota): the app works, it just will not remember the pairing.
  }
}

export function loadPcs(): SavedPcs {
  const saved = read(KEY) as Partial<SavedPcs> | null;
  const list = Array.isArray(saved?.pcs) ? saved.pcs.map(parseConn).filter((c) => c !== null) : [];
  if (list.length > 0) {
    const current = list.some((c) => c.deviceId === saved?.current) ? (saved?.current as string) : list[0]!.deviceId;
    return { current, pcs: list };
  }
  const old = parseConn(read(OLD_KEY));
  return old ? { current: old.deviceId, pcs: [old] } : { current: null, pcs: [] };
}

/** The PC the phone connects to. */
export function loadConn(): SavedConn | null {
  const { current, pcs } = loadPcs();
  return pcs.find((c) => c.deviceId === current) ?? null;
}

/** A new pairing joins the list and becomes the current one. The same PC (same name) is replaced, not listed twice. */
export function saveConn(c: SavedConn): void {
  const same = (o: SavedConn) => o.deviceId === c.deviceId || o.agentName.toLowerCase() === c.agentName.toLowerCase();
  const { pcs } = loadPcs();
  const at = pcs.findIndex(same);
  const others = pcs.filter((o) => !same(o));
  others.splice(at < 0 ? others.length : at, 0, c);
  write({ current: c.deviceId, pcs: others });
}

export function selectPc(deviceId: string): boolean {
  const { pcs } = loadPcs();
  if (!pcs.some((c) => c.deviceId === deviceId)) return false;
  write({ current: deviceId, pcs });
  return true;
}

/** The name the PC calls itself now. */
export function renameCurrent(name: string): void {
  const { current, pcs } = loadPcs();
  if (!name || !pcs.some((c) => c.deviceId === current && c.agentName !== name)) return;
  write({ current, pcs: pcs.map((c) => (c.deviceId === current ? { ...c, agentName: name } : c)) });
}

/** Forgets the current PC; the first of the others becomes the current one. */
export function clearConn(): void {
  const { current, pcs } = loadPcs();
  const rest = pcs.filter((c) => c.deviceId !== current);
  write({ current: rest[0]?.deviceId ?? null, pcs: rest });
}
