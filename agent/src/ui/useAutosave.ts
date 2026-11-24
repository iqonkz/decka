import { useEffect, useRef } from "react";
import { t } from "@decka/shared";
import { showToast } from "./Toast";

export interface AutosaveControllerOptions<T> {
  initialValue: T;
  onSave: (val: T) => Promise<{ ok: boolean; error?: string }>;
  onRollback: (savedVal: T) => void;
  debounceMs?: number;
  onNotify?: (msg: string, kind: "success" | "danger") => void;
  isEqual?: (a: T, b: T) => boolean;
  successMessage?: string;
  errorMessage?: string;
}

export interface AutosaveController<T> {
  setValue: (next: T) => void;
  dispose: () => void;
  getLastSaved: () => T;
  setLastSaved: (saved: T) => void;
}

export function createAutosaveController<T>(options: AutosaveControllerOptions<T>): AutosaveController<T> {
  const {
    initialValue,
    onSave,
    onRollback,
    debounceMs = 400,
    onNotify,
    isEqual = (a, b) => JSON.stringify(a) === JSON.stringify(b),
    successMessage,
    errorMessage,
  } = options;

  let lastSaved = initialValue;
  let timer: ReturnType<typeof setTimeout> | null = null;

  const notify = (msg: string, kind: "success" | "danger") => {
    if (onNotify) onNotify(msg, kind);
    else showToast(msg, kind);
  };

  const setValue = (next: T) => {
    if (isEqual(next, lastSaved)) {
      if (timer) {
        clearTimeout(timer);
        timer = null;
      }
      return;
    }

    if (timer) clearTimeout(timer);

    timer = setTimeout(async () => {
      const prevSaved = lastSaved;
      try {
        const res = await onSave(next);
        if (res.ok) {
          lastSaved = next;
          notify(successMessage ?? t("common.saved_toast"), "success");
        } else {
          notify(errorMessage ?? res.error ?? t("common.save_failed"), "danger");
          onRollback(prevSaved);
        }
      } catch (err) {
        notify(errorMessage ?? (err instanceof Error ? err.message : String(err)) ?? t("common.save_failed"), "danger");
        onRollback(prevSaved);
      }
    }, debounceMs);
  };

  const dispose = () => {
    if (timer) {
      clearTimeout(timer);
      timer = null;
    }
  };

  return {
    setValue,
    dispose,
    getLastSaved: () => lastSaved,
    setLastSaved: (saved: T) => {
      lastSaved = saved;
    },
  };
}

export interface AutosaveOptions<T> {
  value: T;
  onSave: (val: T) => Promise<{ ok: boolean; error?: string }>;
  onRollback: (savedVal: T) => void;
  debounceMs?: number;
  enabled?: boolean;
  successMessage?: string;
  errorMessage?: string;
  isEqual?: (a: T, b: T) => boolean;
}

export function useAutosave<T>({
  value,
  onSave,
  onRollback,
  debounceMs = 400,
  enabled = true,
  successMessage,
  errorMessage,
  isEqual,
}: AutosaveOptions<T>) {
  const controllerRef = useRef<AutosaveController<T> | null>(null);
  const wasEnabledRef = useRef(enabled);

  if (!controllerRef.current) {
    controllerRef.current = createAutosaveController({
      initialValue: value,
      onSave,
      onRollback,
      debounceMs,
      isEqual,
      successMessage,
      errorMessage,
    });
  }

  useEffect(() => {
    if (!wasEnabledRef.current && enabled) {
      wasEnabledRef.current = true;
      controllerRef.current?.setLastSaved(value);
      return;
    }
    wasEnabledRef.current = enabled;
    if (enabled) {
      controllerRef.current?.setValue(value);
    }
  }, [value, enabled]);

  useEffect(() => {
    return () => {
      controllerRef.current?.dispose();
    };
  }, []);

  return controllerRef.current;
}
