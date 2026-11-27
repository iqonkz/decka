#!/usr/bin/env node

/**
 * tools/voice-eval.mjs — Evaluates voice commands or checks evaluation sets.
 *
 * Usage:
 *   node tools/voice-eval.mjs --check
 *   cargo run -q -p decka-core --bin voice-prompt-dump -- --lang en > en-prompt.json
 *   node tools/voice-eval.mjs --prompt en-prompt.json --port 8089 [--api-key <key>] [--url <http://...>] [--lang <en>]
 *   (the prompt file is required: the request is built from the agent's own prompt, not a stand-in)
 *
 * (DATA_MODEL.md §12.8, TASKS.md §0 / T21.4)
 */

import fs from "fs";
import path from "path";
import { fileURLToPath } from "url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const REPO_ROOT = path.resolve(__dirname, "..");
const EVAL_DIR = path.join(REPO_ROOT, "shared", "voice", "eval");

// Tools permitted by DATA_MODEL.md §12.5
export const VALID_TOOLS = new Set([
  "press_button",
  "run_action",
  "open_app",
  "switch_app",
  "close_app",
  "window_other_monitor",
  "which_monitor",
  "volume",
  "media",
  "go_page",
  "web_search",
  "open_site",
  "type_text",
  "dictate",
  "timer",
  "system",
  // Later stages (DATA_MODEL.md 12.5): text work, screen help, reminders, lists, brief, memory, calc, ask.
  "text_transform",
  "screen_ask",
  "remind",
  "reminders_show",
  "list_add",
  "list_show",
  "list_remove",
  "day_brief",
  "memory_add",
  "memory_remove",
  "calc",
  "ask",
  "lyrics_window",
  "status",
  "reply_action",
  // M27: screen highlight, UI actions, step-by-step guide, read aloud, work modes
  "find_on_screen",
  "ui_act",
  "guide",
  "guide_record",
  "read_aloud",
  "mode_run",
  "mode_save",
  "screenshot",
  "add_button",
  "system_theme",
  "notes_record",
  "memory_top",
  "ha",
  "guide_control",
]);

// Accuracy targets per DATA_MODEL.md §12.8:
// en/ru/es/de/fr/pt >= 90%, others >= 80%
const TIER1_LANGS = new Set(["en", "ru", "es", "de", "fr", "pt"]);
function targetAccuracy(lang) {
  return TIER1_LANGS.has(lang) ? 90 : 80;
}

function parseArgs(args) {
  const flags = {
    check: false,
    port: 8080,
    apiKey: process.env.LLAMA_API_KEY || "",
    url: "",
    lang: "",
    prompt: "",
  };

  for (let i = 0; i < args.length; i++) {
    const a = args[i];
    if (a === "--check") {
      flags.check = true;
    } else if (a === "--port" && i + 1 < args.length) {
      flags.port = parseInt(args[++i], 10);
    } else if ((a === "--api-key" || a === "--key") && i + 1 < args.length) {
      flags.apiKey = args[++i];
    } else if (a === "--url" && i + 1 < args.length) {
      flags.url = args[++i];
    } else if (a === "--lang" && i + 1 < args.length) {
      flags.lang = args[++i];
    } else if (a === "--prompt" && i + 1 < args.length) {
      flags.prompt = args[++i];
    }
  }

  if (!flags.url) {
    flags.url = `http://127.0.0.1:${flags.port}/v1/chat/completions`;
  }

  return flags;
}

export function validateEvalFile(filePath) {
  const filename = path.basename(filePath);
  const issues = [];

  let content;
  try {
    content = fs.readFileSync(filePath, "utf8");
  } catch (err) {
    return { ok: false, count: 0, issues: [`Cannot read file: ${err.message}`] };
  }

  let items;
  try {
    items = JSON.parse(content);
  } catch (err) {
    return { ok: false, count: 0, issues: [`Invalid JSON: ${err.message}`] };
  }

  if (!Array.isArray(items)) {
    return { ok: false, count: 0, issues: ["Root must be an array of eval phrases"] };
  }

  items.forEach((item, idx) => {
    const prefix = `[item ${idx}]`;
    if (!item || typeof item !== "object") {
      issues.push(`${prefix} item must be an object`);
      return;
    }
    if (typeof item.say !== "string" || !item.say.trim()) {
      issues.push(`${prefix} 'say' must be a non-empty string`);
    }
    if (typeof item.tool !== "string" || !VALID_TOOLS.has(item.tool)) {
      issues.push(
        `${prefix} invalid tool '${item.tool}'. Allowed: ${Array.from(VALID_TOOLS).join(", ")}`
      );
    }
    if (item.args !== undefined && (typeof item.args !== "object" || item.args === null || Array.isArray(item.args))) {
      issues.push(`${prefix} 'args' must be an object`);
    }
  });

  return {
    ok: issues.length === 0,
    count: items.length,
    issues,
  };
}

async function runCheck() {
  if (!fs.existsSync(EVAL_DIR)) {
    console.error(`Eval directory not found: ${EVAL_DIR}`);
    process.exit(1);
  }

  const files = fs.readdirSync(EVAL_DIR).filter((f) => f.endsWith(".json"));
  if (files.length === 0) {
    console.error(`No eval files found in ${EVAL_DIR}`);
    process.exit(1);
  }

  console.log(`Checking ${files.length} eval file(s) in ${EVAL_DIR}...\n`);
  let hasErrors = false;

  for (const file of files) {
    const fullPath = path.join(EVAL_DIR, file);
    const result = validateEvalFile(fullPath);

    if (result.ok) {
      console.log(`  ✓ ${file.padEnd(12)} ${result.count} phrases (valid)`);
    } else {
      hasErrors = true;
      console.error(`  ✗ ${file.padEnd(12)} FAILED (${result.issues.length} issue(s)):`);
      result.issues.forEach((iss) => console.error(`      - ${iss}`));
    }
  }

  console.log();
  if (hasErrors) {
    console.error("Validation failed with errors.");
    process.exit(1);
  } else {
    console.log("All eval sets are valid.");
    process.exit(0);
  }
}

export function loadPromptDump(file) {
  if (!file) {
    throw new Error("--prompt <file> is required: run `cargo run -q -p decka-core --bin voice-prompt-dump -- --lang <code>` and save its output");
  }
  // PowerShell 5.1 writes a BOM with Out-File / `>`; JSON.parse does not accept it.
  const dump = JSON.parse(fs.readFileSync(file, "utf8").replace(/^﻿/, ""));
  if (typeof dump.system !== "string" || !Array.isArray(dump.tools) || typeof dump.params !== "object" || dump.params === null) {
    throw new Error(`${file}: not a voice-prompt-dump output ({ system, tools, params })`);
  }
  return dump;
}

async function queryModel(url, apiKey, prompt, dump) {
  const headers = { "Content-Type": "application/json" };
  if (apiKey) {
    headers["Authorization"] = `Bearer ${apiKey}`;
  }

  // The request of the agent: its system prompt and its params (schema, temperature, no thinking), plus the phrase.
  const body = {
    ...dump.params,
    messages: [
      { role: "system", content: dump.system },
      { role: "user", content: prompt },
    ],
  };

  const res = await fetch(url, {
    method: "POST",
    headers,
    body: JSON.stringify(body),
  });

  if (!res.ok) {
    throw new Error(`HTTP ${res.status}: ${await res.text()}`);
  }

  const json = await res.json();
  const text = json.choices?.[0]?.message?.content || "{}";
  return JSON.parse(text);
}

function matchArgs(expected, actual) {
  if (!expected || Object.keys(expected).length === 0) return true;
  if (!actual || typeof actual !== "object") return false;
  for (const [k, v] of Object.entries(expected)) {
    if (actual[k] !== v && String(actual[k]).toLowerCase() !== String(v).toLowerCase()) {
      return false;
    }
  }
  return true;
}

async function runEval(flags) {
  if (!fs.existsSync(EVAL_DIR)) {
    console.error(`Eval directory not found: ${EVAL_DIR}`);
    process.exit(1);
  }

  let files = fs.readdirSync(EVAL_DIR).filter((f) => f.endsWith(".json"));
  if (flags.lang) {
    files = files.filter((f) => f.startsWith(flags.lang));
  }

  if (files.length === 0) {
    console.error(`No matching eval files found in ${EVAL_DIR}`);
    process.exit(1);
  }

  let dump;
  try {
    dump = loadPromptDump(flags.prompt);
  } catch (err) {
    console.error(`Error: ${err.message}`);
    process.exit(2);
  }
  console.log(`Evaluating against ${flags.url}...\n`);
  const results = [];
  const misses = [];

  for (const file of files) {
    const lang = file.replace(".json", "");
    const target = targetAccuracy(lang);
    const fullPath = path.join(EVAL_DIR, file);
    const items = JSON.parse(fs.readFileSync(fullPath, "utf8"));

    let correct = 0;
    process.stdout.write(`Evaluating ${lang} (${items.length} items)... `);

    for (const item of items) {
      try {
        const pred = await queryModel(flags.url, flags.apiKey, item.say, dump);
        const toolMatch = pred.tool === item.tool;
        const argsMatch = matchArgs(item.args, pred.args);
        if (toolMatch && argsMatch) {
          correct++;
        } else {
          misses.push({ lang, say: item.say, want: `${item.tool} ${JSON.stringify(item.args || {})}`, got: `${pred.tool} ${JSON.stringify(pred.args || {})}` });
        }
      } catch (err) {
        misses.push({ lang, say: item.say, want: item.tool, got: `error: ${err.message}`.slice(0, 80) });
      }
    }

    const accuracy = items.length > 0 ? (correct / items.length) * 100 : 0;
    const passed = accuracy >= target;
    results.push({ lang, total: items.length, correct, accuracy, target, passed });
    console.log(`${accuracy.toFixed(1)}% (${passed ? "PASS" : "FAIL"})`);
  }

  // Print results summary table
  console.log("\n" + "=".repeat(62));
  console.log(
    "Lang".padEnd(8) +
      "Total".padEnd(8) +
      "Correct".padEnd(10) +
      "Accuracy".padEnd(12) +
      "Target".padEnd(12) +
      "Status"
  );
  console.log("-".repeat(62));

  let allPass = true;
  for (const r of results) {
    if (!r.passed) allPass = false;
    console.log(
      r.lang.padEnd(8) +
        String(r.total).padEnd(8) +
        String(r.correct).padEnd(10) +
        `${r.accuracy.toFixed(1)}%`.padEnd(12) +
        `>=${r.target}%`.padEnd(12) +
        (r.passed ? "PASS" : "FAIL")
    );
  }
  console.log("=".repeat(62) + "\n");

  if (misses.length) {
    // The most frequent wrong answers first: which expected tool was taken for which.
    const counts = new Map();
    for (const m of misses) {
      const k = `${m.want.split(" ")[0]} -> ${m.got.split(" ")[0]}`;
      counts.set(k, (counts.get(k) || 0) + 1);
    }
    console.log("Top confusions (expected -> got):");
    [...counts.entries()].sort((a, b) => b[1] - a[1]).slice(0, 5).forEach(([k, n]) => console.log(`  ${n}x ${k}`));
    console.log("First misses:");
    misses.slice(0, 5).forEach((m) => console.log(`  [${m.lang}] "${m.say}": want ${m.want}; got ${m.got}`));
    console.log();
  }

  if (!allPass) {
    console.error("Some languages did not achieve required accuracy.");
    process.exit(1);
  }
  console.log("All languages passed evaluation target.");
  process.exit(0);
}

async function main() {
  const flags = parseArgs(process.argv.slice(2));
  if (flags.check) {
    await runCheck();
  } else {
    await runEval(flags);
  }
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  main().catch((err) => {
    console.error("Fatal error:", err);
    process.exit(1);
  });
}
