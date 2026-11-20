import { describe, expect, test } from "vitest";
import { hexToBytes, bytesToHex } from "@noble/hashes/utils.js";
import fixture from "@decka/shared/fixtures/crypto-v2.json";
import { Opener, Sealer, deriveKeys, fromB64u, kDev, mac, macEquals, sCode, toB64u } from "../src/net/secure";

const enc = new TextEncoder();
const cn = fromB64u(fixture.cn) as Uint8Array;
const sn = fromB64u(fixture.sn) as Uint8Array;

describe("crypto v2 shared vectors", () => {
  test("K_dev and S_code", () => {
    expect(bytesToHex(kDev(fixture.token))).toBe(fixture.kDev);
    expect(bytesToHex(sCode(fixture.code, cn, sn))).toBe(fixture.sCode);
    expect(bytesToHex(sCode(fixture.code.toLowerCase(), cn, sn))).toBe(fixture.sCode);
  });

  test.each(["device", "pair_qr", "pair_code"] as const)("keys and both MACs in mode %s", (mode) => {
    const m = fixture.modes[mode];
    const keys = deriveKeys(hexToBytes(m.ikm), cn, sn);
    const okm = new Uint8Array([...keys.auth, ...keys.c2s, ...keys.s2c]);
    expect(bytesToHex(okm)).toBe(m.okm);
    expect(toB64u(mac(keys.auth, "C", cn, sn))).toBe(m.macC);
    expect(toB64u(mac(keys.auth, "S", cn, sn))).toBe(m.macS);
    expect(macEquals(mac(keys.auth, "C", cn, sn), fromB64u(m.macC) as Uint8Array)).toBe(true);
    expect(macEquals(mac(keys.auth, "S", cn, sn), fromB64u(m.macC) as Uint8Array)).toBe(false);
  });

  test("frames match the vectors and open", () => {
    const keys = deriveKeys(hexToBytes(fixture.kDev), cn, sn);
    const sealer = new Sealer(keys.c2s);
    const opener = new Opener(keys.c2s);
    for (const f of fixture.frames.c2s) {
      expect(bytesToHex(sealer.seal(enc.encode(f.plain)))).toBe(f.hex);
      const plain = opener.open(hexToBytes(f.hex));
      expect(new TextDecoder().decode(plain as Uint8Array)).toBe(f.plain);
    }
    const s = fixture.frames.s2c[0] as { plain: string; hex: string };
    expect(bytesToHex(new Sealer(keys.s2c).seal(enc.encode(s.plain)))).toBe(s.hex);
  });
});

describe("frame checks", () => {
  const key = new Uint8Array(32).fill(7);
  test("replay, gap, changed byte, wrong key and short frames are refused", () => {
    const s = new Sealer(key);
    const [f0, f1, f2] = [s.seal(enc.encode("zero")), s.seal(enc.encode("one")), s.seal(enc.encode("two"))] as [Uint8Array, Uint8Array, Uint8Array];
    const o = new Opener(key);
    expect(o.open(f0)).not.toBeNull();
    expect(o.open(f0)).toBeNull(); // replay
    expect(o.open(f2)).toBeNull(); // gap
    expect(o.open(f1)).not.toBeNull();

    const tampered = f0.slice();
    tampered[tampered.length - 1] = (tampered[tampered.length - 1] as number) ^ 1;
    expect(new Opener(key).open(tampered)).toBeNull();
    expect(new Opener(new Uint8Array(32).fill(8)).open(f0)).toBeNull();
    expect(new Opener(key).open(f0.slice(0, 10))).toBeNull();
  });

  test("base64url round trip and bad input", () => {
    const bytes = Uint8Array.from([0, 250, 251, 252, 253, 254, 255, 1, 2]);
    expect(fromB64u(toB64u(bytes))).toEqual(bytes);
    expect(fromB64u("a+b")).toBeNull();
    expect(fromB64u("a")).toBeNull();
  });
});
