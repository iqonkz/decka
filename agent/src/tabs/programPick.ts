import type { Draft } from "./actionForm";

/**
 * Fills a `launch` draft from a program chosen in the file dialog: `Program` always, `Name` (file name without
 * `.exe`) and `Working directory` (the file's folder) only when they are still empty.
 */
export function applyPickedProgram(draft: Draft, path: string): Draft {
  const slash = Math.max(path.lastIndexOf("\\"), path.lastIndexOf("/"));
  const file = path.slice(slash + 1);
  let dir = slash > 0 ? path.slice(0, slash) : "";
  if (/^[A-Za-z]:$/.test(dir)) dir += "\\"; // "C:" alone would mean the current folder of drive C
  return {
    ...draft,
    program: path,
    name: draft.name.trim() === "" ? file.replace(/\.exe$/i, "") : draft.name,
    cwd: draft.cwd.trim() === "" ? dir : draft.cwd,
  };
}
