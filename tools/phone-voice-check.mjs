#!/usr/bin/env node
// T32.H.3: the whole chain of a phone voice request on this PC, doing nothing for real. Starts the dry-run copy of the
// agent (`cargo build -p decka-core --example phone_voice`) with a settings folder of its own, pairs as a phone, sends the
// WAV file as the phone does and expects the PC to say "done" and to print `DRY voice launch`.
//   node tools/phone-voice-check.mjs <folder of the voice package> <say.wav> [lang] [port]
// The WAV file (16 kHz, mono, 16 bit) should ask to open a program, e.g. "открой калькулятор".
import { spawn, spawnSync } from "node:child_process";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const [aiRoot, wav, lang = "ru", port = "19252"] = process.argv.slice(2);
if (!aiRoot || !wav) {
  console.error("usage: phone-voice-check.mjs <folder of the voice package> <say.wav> [lang] [port]");
  process.exit(64);
}
const tools = dirname(fileURLToPath(import.meta.url));
const exe = join(tools, "..", "target", "debug", "examples", "phone_voice.exe");
const config = mkdtempSync(join(tmpdir(), "decka-phone-voice-"));
const code = "ABCD2345";

const agent = spawn(exe, ["--ai-root", resolve(aiRoot), "--port", port, "--pair-code", code, "--lang", lang], {
  env: { ...process.env, DECKA_CONFIG_DIR: config, DECKA_VOICE_WAV: resolve(wav) },
  stdio: ["ignore", "pipe", "inherit"],
});
let printed = "";
const ready = new Promise((done, failed) => {
  agent.stdout.on("data", (data) => {
    printed += data;
    if (printed.includes("READY Ready")) done();
  });
  agent.on("error", failed);
  agent.on("exit", (status) => failed(new Error(`the agent ended (${status}): ${printed}`)));
  setTimeout(() => failed(new Error(`the agent is not ready, is the voice package complete? ${printed}`)), 30000);
});

function smoke(...args) {
  const run = spawnSync(process.execPath, [join(tools, "ws-smoke.mjs"), ...args], { encoding: "utf8", timeout: 240000 });
  process.stdout.write(run.stdout);
  process.stderr.write(run.stderr);
  return run;
}

let failure = null;
try {
  await ready;
  const paired = smoke("pair", `127.0.0.1:${port}`, code);
  if (paired.status !== 0) throw new Error("pairing failed");
  const said = smoke("voice", `127.0.0.1:${port}`, JSON.parse(paired.stdout).token, resolve(wav));
  if (said.status !== 0) throw new Error("the PC did not say \"done\"");
  // The pipe of the agent may be a moment behind its answer to the phone.
  await new Promise((done) => setTimeout(done, 500));
  if (!/^DRY voice launch\r?$/m.test(printed)) throw new Error(`nothing was launched: ${printed}`);
} catch (e) {
  failure = e;
} finally {
  // This process only: its helpers are in its job object and end with it.
  agent.removeAllListeners("exit");
  const gone = new Promise((done) => agent.once("exit", done));
  agent.kill();
  await gone;
  rmSync(config, { recursive: true, force: true, maxRetries: 5, retryDelay: 200 });
}
console.log(failure ? `FAILED: ${failure.message}` : "OK: the phone request launched the program (dry run)");
process.exit(failure ? 1 : 0);
