import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { createAutosaveController } from "./useAutosave";

describe("createAutosaveController", () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("debounces save by 400 ms and ignores identical values", async () => {
    const onSave = vi.fn().mockResolvedValue({ ok: true });
    const onRollback = vi.fn();
    const onNotify = vi.fn();

    const controller = createAutosaveController({
      initialValue: { accent: "#7c5cff" },
      onSave,
      onRollback,
      onNotify,
      debounceMs: 400,
      successMessage: "Saved",
    });

    // Same value -> should not trigger save
    controller.setValue({ accent: "#7c5cff" });
    vi.advanceTimersByTime(500);
    expect(onSave).not.toHaveBeenCalled();

    // Change value
    controller.setValue({ accent: "#3b82f6" });
    vi.advanceTimersByTime(200);
    expect(onSave).not.toHaveBeenCalled();

    // Change value again within 400 ms -> debounce resets
    controller.setValue({ accent: "#ef4444" });
    vi.advanceTimersByTime(200);
    expect(onSave).not.toHaveBeenCalled();

    // Advance remaining 200 ms -> 400 ms elapsed since last change
    await vi.advanceTimersByTimeAsync(200);
    expect(onSave).toHaveBeenCalledTimes(1);
    expect(onSave).toHaveBeenCalledWith({ accent: "#ef4444" });
    expect(onNotify).toHaveBeenCalledWith("Saved", "success");
    expect(controller.getLastSaved()).toEqual({ accent: "#ef4444" });
  });

  it("rolls back to previous saved value and notifies with danger on error", async () => {
    const onSave = vi.fn().mockResolvedValue({ ok: false, error: "Disk full" });
    const onRollback = vi.fn();
    const onNotify = vi.fn();

    const initial = { port: 9247 };
    const controller = createAutosaveController({
      initialValue: initial,
      onSave,
      onRollback,
      onNotify,
      debounceMs: 400,
    });

    controller.setValue({ port: 8080 });
    await vi.advanceTimersByTimeAsync(400);

    expect(onSave).toHaveBeenCalledTimes(1);
    expect(onSave).toHaveBeenCalledWith({ port: 8080 });
    expect(onRollback).toHaveBeenCalledWith(initial);
    expect(onNotify).toHaveBeenCalledWith("Disk full", "danger");
  });

  it("rolls back when onSave rejects with an exception", async () => {
    const onSave = vi.fn().mockRejectedValue(new Error("Network timeout"));
    const onRollback = vi.fn();
    const onNotify = vi.fn();

    const initial = { theme: "amoled" };
    const controller = createAutosaveController({
      initialValue: initial,
      onSave,
      onRollback,
      onNotify,
      debounceMs: 400,
    });

    controller.setValue({ theme: "glass" });
    await vi.advanceTimersByTimeAsync(400);

    expect(onSave).toHaveBeenCalledTimes(1);
    expect(onRollback).toHaveBeenCalledWith(initial);
    expect(onNotify).toHaveBeenCalledWith("Network timeout", "danger");
  });

  it("cancels pending save when disposed", async () => {
    const onSave = vi.fn().mockResolvedValue({ ok: true });
    const onRollback = vi.fn();

    const controller = createAutosaveController({
      initialValue: { val: 1 },
      onSave,
      onRollback,
      debounceMs: 400,
    });

    controller.setValue({ val: 2 });
    vi.advanceTimersByTime(200);
    controller.dispose();
    await vi.advanceTimersByTimeAsync(400);

    expect(onSave).not.toHaveBeenCalled();
  });
});
