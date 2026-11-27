// T32.4: several PCs on one phone.
import { beforeEach, describe, expect, test, vi } from "vitest";
import { pcEntries, probePc } from "../src/net/pcs";
import type { WebSocketCtor } from "../src/net/connection";
import { clearConn, loadConn, loadPcs, renameCurrent, saveConn, selectPc } from "../src/store/storage";

const pc = (n: number, name: string) => ({ token: `t${n}`, deviceId: `d${n}`, hosts: [`192.168.1.${n}`], port: 9247, agentName: name });

describe("paired PCs", () => {
  const data = new Map<string, string>();
  beforeEach(() => {
    data.clear();
    vi.stubGlobal("localStorage", {
      getItem: (k: string) => data.get(k) ?? null,
      setItem: (k: string, v: string) => void data.set(k, v),
      removeItem: (k: string) => void data.delete(k),
    });
  });

  test("the single pairing of an earlier version becomes the list", () => {
    data.set("decka.conn.v1", JSON.stringify(pc(1, "Desk")));
    expect(loadConn()?.token).toBe("t1");
    saveConn(pc(2, "Laptop"));
    expect(data.has("decka.conn.v1")).toBe(false);
    expect(loadPcs().pcs.map((p) => p.agentName)).toEqual(["Desk", "Laptop"]);
  });

  test("a new PC joins the list and is current; the same PC is replaced", () => {
    saveConn(pc(1, "Desk"));
    saveConn(pc(2, "Laptop"));
    expect(loadConn()?.agentName).toBe("Laptop");
    saveConn(pc(3, "desk"));
    expect(loadPcs().pcs.map((p) => p.deviceId)).toEqual(["d3", "d2"]);
    expect(loadConn()?.deviceId).toBe("d3");
  });

  test("the chosen PC is remembered; forgetting it leaves the others", () => {
    saveConn(pc(1, "Desk"));
    saveConn(pc(2, "Laptop"));
    expect(selectPc("d1")).toBe(true);
    expect(selectPc("nope")).toBe(false);
    expect(loadConn()?.deviceId).toBe("d1");
    renameCurrent("Study");
    expect(loadConn()?.agentName).toBe("Study");
    clearConn();
    expect(loadConn()?.deviceId).toBe("d2");
    clearConn();
    expect(loadConn()).toBeNull();
    expect(data.size).toBe(0);
  });

  test("the list shows who answers", () => {
    saveConn(pc(1, "Desk"));
    saveConn(pc(2, "Laptop"));
    expect(pcEntries(new Set(["d1"]), false)).toEqual([
      { id: "d1", name: "Desk", address: "192.168.1.1", online: true, current: false },
      { id: "d2", name: "Laptop", address: "192.168.1.2", online: false, current: true },
    ]);
  });

  test("a PC answers when one of its addresses opens, and sends nothing", async () => {
    const sent: string[] = [];
    const make = (open: (url: string) => boolean) =>
      class {
        onopen: (() => void) | null = null;
        onerror: (() => void) | null = null;
        onclose: (() => void) | null = null;
        constructor(url: string) {
          queueMicrotask(() => (open(url) ? this.onopen?.() : this.onerror?.()));
        }
        send(text: string) {
          sent.push(text);
        }
        close() {}
      } as unknown as WebSocketCtor;
    const two = { ...pc(1, "Desk"), hosts: ["10.0.0.9", "192.168.1.1"] };
    await expect(probePc(two, make((u) => u === "ws://192.168.1.1:9247/ws"))).resolves.toBe(true);
    await expect(probePc(two, make(() => false))).resolves.toBe(false);
    expect(sent).toEqual([]);
  });
});
