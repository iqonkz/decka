import { sha256 } from "@noble/hashes/sha2.js";

/** Bytes per `apk_chunk` (PROTOCOL.md 3). */
export const APK_CHUNK = 3000;

export type ApkFailure = "BAD_CHUNK" | "TOO_BIG" | "HASH_MISMATCH";

export class ApkError extends Error {
  constructor(readonly code: ApkFailure) {
    super(code);
  }
}

const MAX_APK = 150 * 1024 * 1024;

/** Lower-case hex of `bytes`. */
export function toHex(bytes: Uint8Array): string {
  return Array.from(bytes, (b) => b.toString(16).padStart(2, "0")).join("");
}

/**
 * Puts the pieces of the new APK together, in order, and checks the SHA-256 of the whole file against the one the agent announced
 * in the section `update`. Nothing is handed on (`finish`) unless the hash matches.
 */
export class ApkAssembler {
  private parts: Uint8Array[] = [];
  private received = 0;
  private readonly hash = sha256.create();

  constructor(
    readonly total: number,
    private readonly expected: string,
  ) {
    if (!Number.isInteger(total) || total <= 0 || total > MAX_APK) throw new ApkError("TOO_BIG");
  }

  /** The offset of the next piece to ask for, or null when everything has arrived. */
  get next(): number | null {
    return this.received >= this.total ? null : this.received;
  }

  get percent(): number {
    return Math.min(100, Math.floor((this.received / this.total) * 100));
  }

  /** Adds the piece at `offset`; it must be the next one, at most 3000 bytes, and full unless it is the last. */
  add(offset: number, bytes: Uint8Array): void {
    const last = offset + bytes.length === this.total;
    if (offset !== this.received || bytes.length === 0 || bytes.length > APK_CHUNK || (!last && bytes.length !== APK_CHUNK) || offset + bytes.length > this.total) {
      throw new ApkError("BAD_CHUNK");
    }
    this.parts.push(bytes);
    this.hash.update(bytes);
    this.received += bytes.length;
  }

  /** The whole file; throws `HASH_MISMATCH` (and keeps nothing) when it is not the file the agent announced. */
  finish(): Uint8Array {
    if (this.received !== this.total) throw new ApkError("BAD_CHUNK");
    if (toHex(this.hash.digest()) !== this.expected.toLowerCase()) {
      this.parts = [];
      throw new ApkError("HASH_MISMATCH");
    }
    const out = new Uint8Array(this.total);
    let at = 0;
    for (const part of this.parts) {
      out.set(part, at);
      at += part.length;
    }
    return out;
  }
}

/** Whether `latest` is a stable `X.Y.Z` newer than `current` (the phone offers an update only for a newer one). */
export function isNewerVersion(latest: string, current: string): boolean {
  const re = /^\d{1,9}\.\d{1,9}\.\d{1,9}$/;
  if (!re.test(latest) || !re.test(current)) return false;
  const a = latest.split(".").map(Number);
  const b = current.split(".").map(Number);
  for (let i = 0; i < 3; i++) if (a[i] !== b[i]) return (a[i] ?? 0) > (b[i] ?? 0);
  return false;
}
