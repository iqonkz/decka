import { describe, expect, test } from "vitest";
import { sha256 } from "@noble/hashes/sha2.js";
import { APK_CHUNK, ApkAssembler, ApkError, isNewerVersion, toHex } from "../src/update/apkAssembler";
import SUMS from "../../shared/fixtures/apk-update/SHA256SUMS.txt?raw";

// The fixture of the agent's tests (agent/core/tests/apk_update.rs): a 7001-byte stand-in for the APK, bytes (i * 31 + 7) % 251,
// and its SHA256SUMS.txt (the test below checks that this is the same file).
const APK = Uint8Array.from({ length: 7001 }, (_, i) => (i * 31 + 7) % 251);
const HASH = (SUMS.split("\n").find((line: string) => line.endsWith("Decka-9.9.0.apk")) ?? "").split("  ")[0] ?? "";

type Piece = { offset: number; data: Uint8Array };
const pieces = (bytes: Uint8Array): Piece[] => {
  const out: Piece[] = [];
  for (let offset = 0; offset < bytes.length; offset += APK_CHUNK) out.push({ offset, data: bytes.slice(offset, offset + APK_CHUNK) });
  return out;
};

describe("the new APK from pieces (T20.4)", () => {
  test("the fixture hash is the SHA-256 of the fixture file", () => {
    expect(HASH).toHaveLength(64);
    expect(toHex(sha256(APK))).toBe(HASH);
  });

  test("three pieces of 3000, 3000 and 1001 bytes rebuild the file and pass the SHA-256 check", () => {
    const assembler = new ApkAssembler(APK.length, HASH);
    const all = pieces(APK);
    const seen: number[] = [];
    for (let offset = assembler.next; offset !== null; offset = assembler.next) {
      seen.push(offset);
      const piece = all.find((p) => p.offset === offset);
      if (!piece) throw new Error("no piece");
      assembler.add(piece.offset, piece.data);
    }
    expect(seen).toEqual([0, 3000, 6000]);
    expect(all.map((p) => p.data.length)).toEqual([3000, 3000, 1001]);
    expect(assembler.percent).toBe(100);
    expect(assembler.finish()).toEqual(APK);
  });

  test("a file that is not the announced one is refused with HASH_MISMATCH and nothing is returned", () => {
    const changed = APK.slice();
    changed[4000] = (changed[4000] ?? 0) ^ 0xff;
    const assembler = new ApkAssembler(changed.length, HASH);
    for (const p of pieces(changed)) assembler.add(p.offset, p.data);
    let failure: unknown;
    try {
      assembler.finish();
    } catch (e) {
      failure = e;
    }
    expect(failure).toBeInstanceOf(ApkError);
    expect((failure as ApkError).code).toBe("HASH_MISMATCH");
  });

  test("pieces out of order, too big, short in the middle or past the end are refused", () => {
    const all = pieces(APK);
    const second = all[1];
    const first = all[0];
    if (!first || !second) throw new Error("fixture");
    expect(() => new ApkAssembler(APK.length, HASH).add(second.offset, second.data)).toThrowError("BAD_CHUNK");
    expect(() => new ApkAssembler(APK.length, HASH).add(0, new Uint8Array(APK_CHUNK + 1))).toThrowError("BAD_CHUNK");
    expect(() => new ApkAssembler(APK.length, HASH).add(0, first.data.slice(0, 100))).toThrowError("BAD_CHUNK");
    expect(() => new ApkAssembler(APK.length, HASH).add(0, new Uint8Array(0))).toThrowError("BAD_CHUNK");
    expect(() => new ApkAssembler(APK.length, HASH).finish()).toThrowError("BAD_CHUNK");
  });

  test("an absurd size is refused at once", () => {
    expect(() => new ApkAssembler(0, HASH)).toThrowError("TOO_BIG");
    expect(() => new ApkAssembler(200 * 1024 * 1024, HASH)).toThrowError("TOO_BIG");
  });

  test("a new app is offered only for a newer stable version", () => {
    expect(isNewerVersion("0.11.0", "0.10.2")).toBe(true);
    expect(isNewerVersion("0.10.2", "0.10.2")).toBe(false);
    expect(isNewerVersion("0.10.1", "0.10.2")).toBe(false);
    expect(isNewerVersion("1.0.0-beta.1", "0.10.2")).toBe(false);
  });
});
