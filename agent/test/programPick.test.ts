import { describe, expect, test } from "vitest";
import { emptyDraft } from "../src/tabs/actionForm";
import type { InstalledApp } from "@decka/shared";
import { applyInstalledApp, applyPickedProgram, filterApps } from "../src/tabs/programPick";

const app = (name: string, over: Partial<InstalledApp> = {}): InstalledApp => ({ name, program: `C:\\Apps\\${name}.exe`, args: [], ...over });

describe("applyInstalledApp", () => {
  test("fills program, arguments (one per line), working directory and the shortcut name", () => {
    const d = applyInstalledApp(emptyDraft(), app("Chrome", { args: ["--profile-directory=Default", "--new-window"], cwd: "C:\\Apps" }));
    expect(d.program).toBe("C:\\Apps\\Chrome.exe");
    expect(d.argsText).toBe("--profile-directory=Default\n--new-window");
    expect(d.cwd).toBe("C:\\Apps");
    expect(d.name).toBe("Chrome");
  });

  test("keeps a name that is already set and clears stale arguments", () => {
    const d = applyInstalledApp({ ...emptyDraft(), name: "My browser", argsText: "old", cwd: "D:\\x" }, app("Chrome"));
    expect(d.name).toBe("My browser");
    expect(d.argsText).toBe("");
    expect(d.cwd).toBe("");
  });
});

describe("filterApps", () => {
  const list = [app("Visual Studio Code"), app("Notepad"), app("Steam")];
  test("matches a substring of the name, ignoring case", () => {
    expect(filterApps(list, "STUDIO").map((a) => a.name)).toEqual(["Visual Studio Code"]);
    expect(filterApps(list, "te").map((a) => a.name)).toEqual(["Notepad", "Steam"]);
  });
  test("an empty query keeps everything; the path is not searched", () => {
    expect(filterApps(list, "  ")).toHaveLength(3);
    expect(filterApps(list, "Apps")).toHaveLength(0);
  });
});

describe("applyPickedProgram", () => {
  test("fills program, name and working directory into an empty draft", () => {
    const d = applyPickedProgram(emptyDraft(), "C:\\Windows\\System32\\charmap.exe");
    expect(d.program).toBe("C:\\Windows\\System32\\charmap.exe");
    expect(d.name).toBe("charmap");
    expect(d.cwd).toBe("C:\\Windows\\System32");
  });

  test("keeps a name and a working directory that are already set", () => {
    const d = applyPickedProgram({ ...emptyDraft(), name: "Map", cwd: "D:\\work" }, "C:\\Windows\\System32\\charmap.exe");
    expect(d.name).toBe("Map");
    expect(d.cwd).toBe("D:\\work");
    expect(d.program).toBe("C:\\Windows\\System32\\charmap.exe");
  });

  test("a program in a drive root keeps the backslash; the extension is dropped case-insensitively", () => {
    const d = applyPickedProgram(emptyDraft(), "C:\\TOOL.EXE");
    expect(d.name).toBe("TOOL");
    expect(d.cwd).toBe("C:\\");
  });
});
