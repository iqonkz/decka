#!/usr/bin/env node
// Guards the IPC convention (T21.9): the parameters of every #[tauri::command] are flat and their camelCase names equal the keys of
// the arguments in shared/src/ipc.ts (`IpcCommands`). A mismatch breaks the call at run time ("missing required key ...").
//   node tools/check-ipc.mjs            checks every command
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("..", import.meta.url));
const ts = readFileSync(`${root}shared/src/ipc.ts`, "utf8");
const rs = readFileSync(`${root}agent/src-tauri/src/commands.rs`, "utf8");

const camel = (s) => s.replace(/_([a-z0-9])/g, (_, c) => c.toUpperCase());

/** Keys of a TS object type body: `a: T; b?: U` (top level only). */
export function tsKeys(body) {
  const keys = [];
  let depth = 0;
  let start = 0;
  const parts = [];
  for (let i = 0; i < body.length; i++) {
    const c = body[i];
    if ("{[(<".includes(c)) depth++;
    else if ("}])>".includes(c) && !(c === ">" && body[i - 1] === "=")) depth--;
    else if ((c === ";" || c === ",") && depth === 0) {
      parts.push(body.slice(start, i));
      start = i + 1;
    }
  }
  parts.push(body.slice(start));
  for (const p of parts) {
    const m = /^\s*(?:\/\/[^\n]*\n\s*)*(?:readonly\s+)?([A-Za-z_]\w*)\??\s*:/.exec(p);
    if (m) keys.push(m[1]);
  }
  return keys.sort();
}

/** Parameter names of a Rust command, without the injected `app`, `window`, `state`. */
export function rustParams(list) {
  const out = [];
  let depth = 0;
  let cur = "";
  for (const c of list) {
    if ("<([".includes(c)) depth++;
    else if (">)]".includes(c)) depth--;
    if (c === "," && depth === 0) {
      out.push(cur);
      cur = "";
    } else cur += c;
  }
  if (cur.trim()) out.push(cur);
  const injected = /(AppHandle|State<|Window|WebviewWindow)/;
  return out
    .map((p) => p.trim())
    .filter((p) => p && !injected.test(p))
    .map((p) => camel(p.split(":")[0].trim().replace(/^mut\s+/, "")))
    .sort();
}

function tsCommands() {
  const block = /export interface IpcCommands \{([\s\S]*?)\n\}/.exec(ts)?.[1] ?? "";
  const named = new Map();
  for (const m of ts.matchAll(/export interface (\w+) \{([\s\S]*?)\n\}/g)) named.set(m[1], m[2]);
  const cmds = new Map();
  for (const m of block.matchAll(/^\s*(\w+)\(([^)]*)\)\s*:/gm)) {
    const arg = /^\s*\w+\??\s*:\s*(.*)$/s.exec(m[2])?.[1]?.trim();
    if (!arg || arg.startsWith("Record<")) cmds.set(m[1], []);
    else if (arg.startsWith("{")) cmds.set(m[1], tsKeys(arg.slice(1, arg.lastIndexOf("}"))));
    else cmds.set(m[1], named.has(arg) ? tsKeys(named.get(arg)) : null);
  }
  return cmds;
}

export function check() {
  const problems = [];
  const expected = tsCommands();
  for (const m of rs.matchAll(/#\[tauri::command\]\s*pub\s+(?:async\s+)?fn\s+(\w+)\s*\(([\s\S]*?)\)\s*(?:->|\{)/g)) {
    const [, name, params] = m;
    const want = expected.get(name);
    if (want === undefined) continue; // not in IpcCommands (internal command)
    if (want === null) {
      problems.push(`${name}: argument type of ipc.ts is not understood`);
      continue;
    }
    const got = rustParams(params);
    if (got.join(",") !== want.join(",")) problems.push(`${name}: Rust takes [${got}] but ipc.ts sends [${want}]`);
  }
  return problems;
}

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) {
  const problems = check();
  for (const p of problems) console.error(p);
  if (problems.length) process.exit(1);
  console.log("check-ipc: command parameters match shared/src/ipc.ts");
}
