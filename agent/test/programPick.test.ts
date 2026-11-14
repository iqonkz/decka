import { describe, expect, test } from "vitest";
import { emptyDraft } from "../src/tabs/actionForm";
import { applyPickedProgram } from "../src/tabs/programPick";

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
