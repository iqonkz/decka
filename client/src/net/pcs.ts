// T32.4: which of the paired PCs answer now. Only the current one has a real connection; the others are asked whether
// their port opens a WebSocket, which says "Decka is running there" and sends nothing.
import { useEffect, useState } from "react";
import { loadPcs, type SavedConn } from "../store/storage";
import type { WebSocketCtor } from "./connection";

export const PROBE_EVERY_MS = 5000;
export const PROBE_TIMEOUT_MS = 2500;

export interface PcEntry {
  id: string;
  name: string;
  address: string;
  online: boolean;
  current: boolean;
}

function probeHost(address: string, Ctor: WebSocketCtor, timeoutMs: number): Promise<boolean> {
  return new Promise((resolve) => {
    let done = false;
    let socket: InstanceType<WebSocketCtor> | null = null;
    const finish = (ok: boolean) => {
      if (done) return;
      done = true;
      clearTimeout(timer);
      socket?.close();
      resolve(ok);
    };
    const timer = setTimeout(() => finish(false), timeoutMs);
    try {
      socket = new Ctor(`ws://${address}/ws`);
      socket.onopen = () => finish(true);
      socket.onerror = () => finish(false);
      socket.onclose = () => finish(false);
    } catch {
      finish(false); // an address the browser refuses
    }
  });
}

/** True when one of the addresses of the PC answers. */
export async function probePc(pc: SavedConn, Ctor: WebSocketCtor, timeoutMs = PROBE_TIMEOUT_MS): Promise<boolean> {
  if (pc.hosts.length === 0) return false;
  const tries = pc.hosts.map((h) => probeHost(`${h}:${pc.port}`, Ctor, timeoutMs).then((ok) => (ok ? true : Promise.reject(new Error("silent")))));
  return Promise.any(tries).catch(() => false); // no address answered
}

export function pcEntries(online: ReadonlySet<string>, currentOnline: boolean): PcEntry[] {
  const { current, pcs } = loadPcs();
  return pcs.map((pc) => {
    const isCurrent = pc.deviceId === current;
    return {
      id: pc.deviceId,
      name: pc.agentName,
      address: pc.hosts[0] ?? "",
      online: isCurrent ? currentOnline : online.has(pc.deviceId),
      current: isCurrent,
    };
  });
}

/** The list of the paired PCs; while `watch` is true the others are asked every few seconds. */
export function usePcs(watch: boolean, currentOnline: boolean): PcEntry[] {
  const [online, setOnline] = useState<ReadonlySet<string>>(new Set());
  useEffect(() => {
    if (!watch) return;
    let live = true;
    const look = async () => {
      const { current, pcs } = loadPcs();
      const others = pcs.filter((pc) => pc.deviceId !== current);
      const found = await Promise.all(others.map(async (pc) => ((await probePc(pc, WebSocket as unknown as WebSocketCtor)) ? pc.deviceId : null)));
      if (live) setOnline(new Set(found.filter((id) => id !== null)));
    };
    void look();
    const id = window.setInterval(() => void look(), PROBE_EVERY_MS);
    return () => {
      live = false;
      window.clearInterval(id);
    };
  }, [watch]);
  return pcEntries(online, currentOnline);
}
