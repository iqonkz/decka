#!/usr/bin/env node
// Smoke-test client for the Decka WebSocket API. Node >= 22 (global WebSocket), no dependencies.
//   node tools/ws-smoke.mjs pair  <host:port> <code>
//   node tools/ws-smoke.mjs state <host:port> <token>
//   node tools/ws-smoke.mjs press <host:port> <token> <actionId>
//   node tools/ws-smoke.mjs watch <host:port> <token> <seconds>
//   node tools/ws-smoke.mjs raw   <host:port> <file.json> [token]
// Each command prints one JSON line (watch: one per patch) and exits 0; errors go to stderr with exit code 1.
import { readFileSync } from "node:fs";

const CLIENT = { name: "ws-smoke", app: "web", version: "0.0.0" };
const TIMEOUT_MS = 5000;

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
    ws.addEventListener("message", (ev) => this.push({ msg: JSON.parse(String(ev.data)) }));
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

  send(obj) {
    this.ws.send(typeof obj === "string" ? obj : JSON.stringify(obj));
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

async function hello(conn, token) {
  conn.send({ t: "hello", v: 1, token, client: CLIENT });
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
    conn.send({ t: "pair", v: 1, code: args[0], client: CLIENT });
    const paired = await nextReply(conn);
    if (paired.t !== "paired") throw new Error(`expected paired, got ${paired.t}`);
    return print({ deviceId: paired.deviceId, token: paired.token });
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
