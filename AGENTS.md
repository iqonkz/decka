# AGENTS.md — rules for any coding agent (Antigravity, Codex, Gemini, Claude…)

This repository is planned by an architect; you implement tasks from `docs/TASKS.md`. The project documents are in Russian — read them as they are.

## Read first
1. `CLAUDE.md` — the mandatory rules of this repository (they apply to every agent, not only Claude).
2. `docs/QUICKREF.md` — constants, map of the repo, commands.
3. Your task in `docs/TASKS.md`. Tasks marked **[второй агент]** are for you; tasks marked **[Claude]** are not — do not do them.
4. Only the spec sections your task links to. Never read `docs/archive/` as a whole.

## Hard rules (short version of CLAUDE.md)
- The phone sends only an `actionId` (plus the data messages listed in PROTOCOL.md). Never add a way for the phone to send commands, paths, arguments, keys or URLs to run.
- Do not add or upgrade dependencies, and do not change versions.
- Do not edit `docs/` (except `BLOCKERS.md`, `MANUAL_CHECKS.md`, `RECIPES.md`, `TESTING.md`), `CLAUDE.md` or this file. Never stash, revert or overwrite uncommitted changes in `docs/` — they are the architect's.
- Do not touch `%APPDATA%\Decka` or any real user data; tests use temp folders.
- Never print or commit secrets (keys, passwords, tokens).
- Do not copy code from other projects.
- Visible UI strings only through `t()` with all 11 dictionaries in `shared/src/i18n`; code comments in English.
- Shell is Windows PowerShell 5.1: `;` not `&&`, `curl.exe` not `curl`.
- UI tasks without a backend yet: call the IPC commands named in the task through typed wrappers; if the command is missing, show the error state. Never persist settings or lists in `localStorage`/`sessionStorage`, never hardcode data that the PC owns (voice/model lists, sizes, licenses — they come from the manifest via IPC), never swallow errors with an empty `.catch(() => {})`. Demo data belongs only in `tools/voice-shots`.

## How to work
- Work in your own branch / worktree (for voice: `git worktree add ..\Decka-ag -b ag-voice`), never on `main`.
- One task = one commit named `T<number>: <title>` (e.g. `T21.3b: voice phrase tables`). Add only your own paths, never `git add -A`.
- **Quick tasks (docs/TASKS.md §0 item 7d):** aim for about 5 minutes per task. Checks: `npm run typecheck` and the tests of the modules you changed. No release builds (installer, APK), screenshots or full test suites — they run once, in the stabilization task before a release.
- Stuck, or two documents disagree → write it in `docs/BLOCKERS.md` and stop that task. Do not make architecture decisions.
- Report in at most 6 lines: commits, checks, deviations, blockers.
