const KEY = "decka.conn.v1";

export interface SavedConn {
  token: string;
  deviceId: string;
  hosts: string[];
  port: number;
  agentName: string;
}

export function loadConn(): SavedConn | null {
  try {
    const raw = localStorage.getItem(KEY);
    if (!raw) return null;
    const v: unknown = JSON.parse(raw);
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
  } catch {
    return null;
  }
}

export function saveConn(c: SavedConn): void {
  try {
    localStorage.setItem(KEY, JSON.stringify(c));
  } catch {
    // Storage unavailable (private mode, quota): the app works, it just will not remember the pairing.
  }
}

export function clearConn(): void {
  try {
    localStorage.removeItem(KEY);
  } catch {
    // see saveConn
  }
}
