#!/usr/bin/env node
// Smoke-test client for the Decka WebSocket API (protocol v2, PROTOCOL.md 10). Node >= 22 (global WebSocket);
// the crypto comes from @noble/ciphers and @noble/hashes, the same libraries as the phone client.
//   node tools/ws-smoke.mjs pair  <host:port> <code>
//   node tools/ws-smoke.mjs state <host:port> <token>
//   node tools/ws-smoke.mjs press <host:port> <token> <actionId>
//   node tools/ws-smoke.mjs watch <host:port> <token> <seconds>
//   node tools/ws-smoke.mjs raw   <host:port> <file.json> [token]
// Each command prints one JSON line (watch: one per patch) and exits 0; errors go to stderr with exit code 1.
// `pair` prints { deviceId, token } where `token` is "<deviceId>.<token>": a v2 login needs the device id, and the
// other commands take this combined value, so `(pair ...).token` keeps working in the H-* macros.
import { readFileSync } from "node:fs";
import { gcm } from "@noble/ciphers/aes.js";
import { equalBytes } from "@noble/ciphers/utils.js";
import { hkdf } from "@noble/hashes/hkdf.js";
import { hmac } from "@noble/hashes/hmac.js";
import { pbkdf2 } from "@noble/hashes/pbkdf2.js";
import { sha256 } from "@noble/hashes/sha2.js";

const CLIENT = { name: "ws-smoke", app: "web", version: "0.0.0" };
const TIMEOUT_MS = 5000;
const enc = new TextEncoder();
const dec = new TextDecoder();
const b64u = (b) => Buffer.from(b).toString("base64url");
const concat = (...p) => Uint8Array.from(Buffer.concat(p.map((x) => Buffer.from(x))));
const AAD = enc.encode("decka2");

function nonceFor(counter) {
  const n = new Uint8Array(12);
  new DataView(n.buffer).setBigUint64(4, BigInt(counter));
  return n;
}

/** The encrypted channel after the handshake: strict counters in both directions. */
class Channel {
  constructor(keys) {
    this.keys = keys;
    this.out = 0;
    this.in = 0;
  }
  seal(text) {
    const counter = this.out++;
    const head = new Uint8Array(8);
    new DataView(head.buffer).setBigUint64(0, BigInt(counter));
    return concat(head, gcm(this.keys.c2s, nonceFor(counter), AAD).encrypt(enc.encode(text)));
  }
  open(frame) {
    const counter = new DataView(frame.buffer, frame.byteOffset, 8).getBigUint64(0);
    if (counter !== BigInt(this.in)) throw new Error("frame counter mismatch");
    this.in++;
    return dec.decode(gcm(this.keys.s2c, nonceFor(counter), AAD).decrypt(frame.subarray(8)));
  }
}

function fail(message) {
  process.stderr.write(`${message}\n`, () => process.exit(1));
}

function print(value, then = 0) {
  process.stdout.write(`${JSON.stringify(value)}\n`, () => {
    if (then !== null) process.exit(then);
  });
}

class Conn {
  constructor(ws) {
    this.queue = [];
    this.waiter = null;
    this.closed = null;
    this.channel = null;
    ws.binaryType = "arraybuffer";
    ws.addEventListener("message", (ev) => {
      try {
        const text = this.channel ? this.channel.open(new Uint8Array(ev.data)) : String(ev.data);
        this.push({ msg: JSON.parse(text) });
      } catch (e) {
        this.closed = `bad frame: ${e.message}`;
        this.push(null);
      }
    });
    ws.addEventListener("close", (ev) => {
      this.closed = `socket closed (${ev.code})`;
      this.push(null);
    });
    ws.addEventListener("error", () => {
      this.closed = this.closed ?? "socket error";
      this.push(null);
    });
    this.ws = ws;
  }

  push(item) {
    if (this.waiter) {
      const w = this.waiter;
      this.waiter = null;
      w(item);
    } else if (item) {
      this.queue.push(item);
    }
  }

  /** Next server message; rejects on timeout or close. */
  next(timeoutMs = TIMEOUT_MS) {
    if (this.queue.length) return Promise.resolve(this.queue.shift().msg);
    if (this.closed) return Promise.reject(new Error(this.closed));
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => {
        this.waiter = null;
        reject(new Error("timeout waiting for the server"));
      }, timeoutMs);
      this.waiter = (item) => {
        clearTimeout(timer);
        if (item) resolve(item.msg);
        else reject(new Error(this.closed ?? "socket closed"));
      };
    });
  }

  /** After the handshake every frame is encrypted; `raw` strings are sent as the plain text of an encrypted frame. */
  send(obj) {
    const text = typeof obj === "string" ? obj : JSON.stringify(obj);
    this.ws.send(this.channel ? this.channel.seal(text) : text);
  }
}

function connect(hostPort) {
  return new Promise((resolve, reject) => {
    const ws = new WebSocket(`ws://${hostPort}/ws`);
    const conn = new Conn(ws);
    ws.addEventListener("open", () => resolve(conn), { once: true });
    ws.addEventListener("error", () => reject(new Error(`cannot connect to ${hostPort}`)), { once: true });
  });
}

/** Next message that is not a patch; a server `error` becomes an exception. */
async function nextReply(conn, { allowError = false } = {}) {
  for (;;) {
    const msg = await conn.next();
    if (msg.t === "patch") continue;
    if (msg.t === "error" && !allowError) throw new Error(`server error ${msg.code}: ${msg.message}`);
    return msg;
  }
}

/** hello2 -> challenge -> auth -> auth_ok; afterwards the connection is encrypted. `secret` = K_dev, S_qr or a function of (cn, sn). */
async function handshake(conn, mode, secret, deviceId) {
  const cn = crypto.getRandomValues(new Uint8Array(16));
  conn.send({ t: "hello2", v: 2, mode, ...(deviceId ? { deviceId } : {}), cn: b64u(cn), client: CLIENT });
  const challenge = await nextReply(conn);
  if (challenge.t !== "challenge") throw new Error(`expected challenge, got ${challenge.t}`);
  const sn = new Uint8Array(Buffer.from(challenge.sn, "base64url"));
  const ikm = typeof secret === "function" ? secret(cn, sn) : secret;
  const okm = hkdf(sha256, ikm, concat(cn, sn), enc.encode("decka v2 keys"), 96);
  const keys = { auth: okm.slice(0, 32), c2s: okm.slice(32, 64), s2c: okm.slice(64, 96) };
  conn.send({ t: "auth", mac: b64u(hmac(sha256, keys.auth, concat(enc.encode("C"), cn, sn))) });
  const ok = await nextReply(conn);
  if (ok.t !== "auth_ok") throw new Error(`expected auth_ok, got ${ok.t}`);
  const expected = hmac(sha256, keys.auth, concat(enc.encode("S"), cn, sn));
  if (!equalBytes(expected, new Uint8Array(Buffer.from(ok.mac, "base64url")))) throw new Error("the PC did not prove its identity");
  conn.channel = new Channel(keys);
}

/** `login` is "<deviceId>.<token>" as printed by `pair`. */
async function hello(conn, login) {
  const dot = login.indexOf(".");
  if (dot < 1) throw new Error("the token must be <deviceId>.<token> as printed by pair");
  const token = login.slice(dot + 1);
  await handshake(conn, "device", sha256(enc.encode(token)), login.slice(0, dot));
  const welcome = await nextReply(conn);
  if (welcome.t !== "welcome") throw new Error(`expected welcome, got ${welcome.t}`);
  return welcome;
}

function need(args, n, usage) {
  if (args.length < n) throw new Error(`usage: ${usage}`);
}

async function main() {
  const [cmd, hostPort, ...args] = process.argv.slice(2);
  if (!cmd || !hostPort) throw new Error("usage: ws-smoke.mjs pair|state|press|watch|raw <host:port> ...");

  if (cmd === "pair") {
    need(args, 1, "pair <host:port> <code>");
    const conn = await connect(hostPort);
    const code = args[0].toUpperCase().replace(/[-\s]/g, "");
    await handshake(conn, "pair_code", (cn, sn) =>
      pbkdf2(sha256, enc.encode(code), concat(enc.encode("decka-pair-v2"), cn, sn), { c: 200000, dkLen: 32 }),
    );
    const paired = await nextReply(conn);
    if (paired.t !== "paired") throw new Error(`expected paired, got ${paired.t}`);
    return print({ deviceId: paired.deviceId, token: `${paired.deviceId}.${paired.token}` });
  }

  if (cmd === "state") {
    need(args, 1, "state <host:port> <token>");
    const welcome = await hello(await connect(hostPort), args[0]);
    return print(welcome.state);
  }

  if (cmd === "press") {
    need(args, 2, "press <host:port> <token> <actionId>");
    const conn = await connect(hostPort);
    await hello(conn, args[0]);
    conn.send({ t: "press", id: 1, actionId: args[1] });
    const result = await nextReply(conn);
    if (result.t !== "press_result") throw new Error(`expected press_result, got ${result.t}`);
    return print(result);
  }

  if (cmd === "watch") {
    need(args, 2, "watch <host:port> <token> <seconds>");
    const conn = await connect(hostPort);
    await hello(conn, args[0]);
    const until = Date.now() + Number(args[1]) * 1000;
    const lines = [];
    while (Date.now() < until) {
      try {
        const msg = await conn.next(Math.max(1, until - Date.now()));
        if (msg.t === "patch") lines.push(JSON.stringify(msg));
        else if (msg.t === "error") throw new Error(`server error ${msg.code}: ${msg.message}`);
      } catch (e) {
        if (String(e.message).startsWith("timeout")) break;
        throw e;
      }
    }
    process.stdout.write(lines.map((l) => `${l}\n`).join(""), () => process.exit(0));
    return;
  }

  if (cmd === "raw") {
    need(args, 1, "raw <host:port> <file.json> [token]");
    // PowerShell 5.1 mangles quotes in native arguments, so the frame comes from a file (BOM stripped).
    const frame = readFileSync(args[0], "utf8").replace(/^﻿/, "").trim();
    const conn = await connect(hostPort);
    if (args[1]) await hello(conn, args[1]);
    conn.send(frame);
    const reply = await nextReply(conn, { allowError: true });
    // The reply is always printed; a server `error` also gives exit code 1 (T9.4 security check).
    return print(reply, reply.t === "error" ? 1 : 0);
  }

  throw new Error(`unknown command '${cmd}'`);
}

main().catch((e) => fail(e.message));
