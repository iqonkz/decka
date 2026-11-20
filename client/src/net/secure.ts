// Protocol v2 cryptography (PROTOCOL.md section 10). Only @noble/ciphers and @noble/hashes primitives are used;
// the MAC is compared with `equalBytes` of the library. Frames carry a strict counter, so a replay, a gap or a
// reordering is refused.
import { gcm } from "@noble/ciphers/aes.js";
import { equalBytes } from "@noble/ciphers/utils.js";
import { hkdf } from "@noble/hashes/hkdf.js";
import { hmac } from "@noble/hashes/hmac.js";
import { pbkdf2 } from "@noble/hashes/pbkdf2.js";
import { sha256 } from "@noble/hashes/sha2.js";

const enc = new TextEncoder();
const PBKDF2_ITERATIONS = 200_000;
const AAD = enc.encode("decka2");
const TAG_LEN = 16;

export interface Keys {
  auth: Uint8Array;
  c2s: Uint8Array;
  s2c: Uint8Array;
}

export function toB64u(bytes: Uint8Array): string {
  let s = "";
  for (const b of bytes) s += String.fromCharCode(b);
  return btoa(s).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
}

/** `null` for text that is not base64url. */
export function fromB64u(text: string): Uint8Array | null {
  if (!/^[A-Za-z0-9_-]*$/.test(text) || text.length % 4 === 1) return null;
  const padded = text.replace(/-/g, "+").replace(/_/g, "/") + "===".slice((text.length + 3) % 4);
  try {
    const bin = atob(padded);
    return Uint8Array.from(bin, (c) => c.charCodeAt(0));
  } catch {
    return null;
  }
}

/** Fresh random bytes (works in the web mode over plain http too, unlike `crypto.subtle`). */
export function randomBytes(n: number): Uint8Array {
  return crypto.getRandomValues(new Uint8Array(n));
}

function concat(...parts: Uint8Array[]): Uint8Array {
  const out = new Uint8Array(parts.reduce((n, p) => n + p.length, 0));
  let at = 0;
  for (const p of parts) {
    out.set(p, at);
    at += p.length;
  }
  return out;
}

/** `K_dev` = SHA-256 of the device token text. */
export function kDev(token: string): Uint8Array {
  return sha256(enc.encode(token));
}

/** `S_code` = PBKDF2-HMAC-SHA256(CODE, "decka-pair-v2" || cn || sn, 200000 iterations, 32 bytes). Slow on purpose. */
export function sCode(code: string, cn: Uint8Array, sn: Uint8Array): Uint8Array {
  return pbkdf2(sha256, enc.encode(code.toUpperCase()), concat(enc.encode("decka-pair-v2"), cn, sn), { c: PBKDF2_ITERATIONS, dkLen: 32 });
}

/** HKDF-SHA256(salt = cn || sn, ikm, info = "decka v2 keys", 96 bytes) split into three keys. */
export function deriveKeys(ikm: Uint8Array, cn: Uint8Array, sn: Uint8Array): Keys {
  const okm = hkdf(sha256, ikm, concat(cn, sn), enc.encode("decka v2 keys"), 96);
  return { auth: okm.slice(0, 32), c2s: okm.slice(32, 64), s2c: okm.slice(64, 96) };
}

/** `HMAC-SHA256(k_auth, label || cn || sn)`; label is "C" (client) or "S" (server). */
export function mac(kAuth: Uint8Array, label: "C" | "S", cn: Uint8Array, sn: Uint8Array): Uint8Array {
  return hmac(sha256, kAuth, concat(enc.encode(label), cn, sn));
}

/** Library comparison of two MACs. */
export function macEquals(a: Uint8Array, b: Uint8Array): boolean {
  return equalBytes(a, b);
}

function nonceFor(counter: bigint): Uint8Array {
  const nonce = new Uint8Array(12);
  new DataView(nonce.buffer).setBigUint64(4, counter);
  return nonce;
}

/** Encrypts the frames of one direction; the counter starts at 0. */
export class Sealer {
  private counter = 0n;
  constructor(private readonly key: Uint8Array) {}

  /** `counter(8, big-endian) || ciphertext || tag`. */
  seal(plain: Uint8Array): Uint8Array {
    const counter = this.counter++;
    const ct = gcm(this.key, nonceFor(counter), AAD).encrypt(plain);
    const head = new Uint8Array(8);
    new DataView(head.buffer).setBigUint64(0, counter);
    return concat(head, ct);
  }
}

/** Decrypts the frames of one direction and demands exactly the next counter. */
export class Opener {
  private expected = 0n;
  constructor(private readonly key: Uint8Array) {}

  /** `null` on a wrong counter (replay, gap, reorder), a short frame or a failed tag; the connection must then end. */
  open(frame: Uint8Array): Uint8Array | null {
    if (frame.length < 8 + TAG_LEN) return null;
    const counter = new DataView(frame.buffer, frame.byteOffset, 8).getBigUint64(0);
    if (counter !== this.expected) return null;
    try {
      const plain = gcm(this.key, nonceFor(counter), AAD).decrypt(frame.subarray(8));
      this.expected++;
      return plain;
    } catch {
      return null;
    }
  }
}
