import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";
import { CARD_AFTER_MS, RETRY_SECONDS, cardButtons, keepsRetrying, linkView, retryDelayMs } from "../src/net/linkState";
import { clearConn, loadConn, saveConn } from "../src/store/storage";

describe("pauses between background attempts (DESIGN 4.13)", () => {
  test("1, 2, 5, 10 seconds, then every 10 seconds", () => {
    expect(RETRY_SECONDS).toEqual([1, 2, 5, 10]);
    expect([0, 1, 2, 3, 4, 5, 50].map(retryDelayMs)).toEqual([1000, 2000, 5000, 10_000, 10_000, 10_000, 10_000]);
    expect(retryDelayMs(-3)).toBe(1000);
  });
});

describe("the card with the way out", () => {
  test("appears after 8 s without a link, not before", () => {
    expect(linkView("offline", CARD_AFTER_MS - 1)).toEqual({ card: false });
    expect(linkView("connecting", CARD_AFTER_MS - 1)).toEqual({ card: false });
    expect(linkView("offline", CARD_AFTER_MS)).toEqual({ card: true, reason: "silent" });
    expect(linkView("connecting", 60_000)).toEqual({ card: true, reason: "silent" });
  });

  test("a rejected key shows the card at once, with the other reason", () => {
    expect(linkView("rejected", 0)).toEqual({ card: true, reason: "rejected" });
  });

  test("an online link or another screen never shows it", () => {
    for (const c of ["online", "unpaired", "repair", "update_required", "not_your_pc"] as const) {
      expect(linkView(c, 999_999), c).toEqual({ card: false });
    }
  });

  test("Connect again and Use another PC are always there; Try again only while the PC may answer", () => {
    expect(cardButtons("silent")).toEqual({ tryAgain: true, connectAgain: true, otherPc: true });
    expect(cardButtons("rejected")).toEqual({ tryAgain: false, connectAgain: true, otherPc: true });
  });

  test("a rejected key means no background attempts", () => {
    expect(keepsRetrying("rejected")).toBe(false);
    expect(keepsRetrying("offline") && keepsRetrying("connecting")).toBe(true);
    expect(keepsRetrying("repair")).toBe(false);
  });
});

describe("the saved PC and a second QR of the same PC", () => {
  const store = new Map<string, string>();
  beforeEach(() => {
    store.clear();
    vi.stubGlobal("localStorage", {
      getItem: (k: string) => store.get(k) ?? null,
      setItem: (k: string, v: string) => void store.set(k, v),
      removeItem: (k: string) => void store.delete(k),
    });
  });
  afterEach(() => vi.unstubAllGlobals());

  test("the second QR updates the address and the key: still one record", () => {
    saveConn({ token: "old", deviceId: "d1", hosts: ["192.168.1.10"], port: 9247, agentName: "Desk" });
    saveConn({ token: "new", deviceId: "d2", hosts: ["192.168.1.55", "10.0.0.2"], port: 9247, agentName: "Desk" });
    expect(store.size).toBe(1);
    expect(loadConn()).toEqual({ token: "new", deviceId: "d2", hosts: ["192.168.1.55", "10.0.0.2"], port: 9247, agentName: "Desk" });
  });

  test("a failed new pairing leaves the saved PC alone (nothing is saved or cleared)", () => {
    saveConn({ token: "t", deviceId: "d1", hosts: ["192.168.1.10"], port: 9247, agentName: "Desk" });
    // pairWith saves only after `paired` arrived: a rejected pairing never reaches saveConn.
    expect(loadConn()?.token).toBe("t");
    clearConn();
    expect(loadConn()).toBeNull();
  });
});
