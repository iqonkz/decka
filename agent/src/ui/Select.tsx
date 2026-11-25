import { useEffect, useId, useRef, useState } from "react";
import PhIcon from "./PhIcon";

export interface SelectOption {
  value: string;
  label: string;
  disabled?: boolean;
}

export interface SelectProps {
  value: string;
  options: readonly SelectOption[];
  onChange: (value: string) => void;
  disabled?: boolean;
  placeholder?: string;
  "aria-label"?: string;
  id?: string;
  className?: string;
  autoFocus?: boolean;
}

/** Custom dropdown select matching the shared field style (DESIGN.md §4.15). */
export default function Select({
  value,
  options,
  onChange,
  disabled = false,
  placeholder,
  "aria-label": ariaLabel,
  id,
  className,
  autoFocus,
}: SelectProps) {
  const [open, setOpen] = useState(false);
  const rootRef = useRef<HTMLDivElement>(null);
  const listId = useId();
  const current = options.find((o) => o.value === value);

  useEffect(() => {
    if (!open) return;
    const onPointerDown = (e: PointerEvent) => {
      if (rootRef.current && !rootRef.current.contains(e.target as Node)) setOpen(false);
    };
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") setOpen(false);
    };
    window.addEventListener("pointerdown", onPointerDown);
    window.addEventListener("keydown", onKeyDown);
    return () => {
      window.removeEventListener("pointerdown", onPointerDown);
      window.removeEventListener("keydown", onKeyDown);
    };
  }, [open]);

  function stepSelection(delta: number) {
    const enabled = options.filter((o) => !o.disabled);
    if (enabled.length === 0) return;
    const idx = enabled.findIndex((o) => o.value === value);
    const next = idx === -1 ? 0 : (idx + delta + enabled.length) % enabled.length;
    const nextOption = enabled[next];
    if (nextOption) onChange(nextOption.value);
  }

  return (
    <div ref={rootRef} className={`custom-select${open ? " open" : ""}${className ? ` ${className}` : ""}`}>
      <button
        type="button"
        id={id}
        autoFocus={autoFocus}
        disabled={disabled}
        role="combobox"
        aria-expanded={open}
        aria-controls={listId}
        aria-label={ariaLabel}
        className="custom-select-trigger"
        onClick={() => !disabled && setOpen((v) => !v)}
        onKeyDown={(e) => {
          if (disabled) return;
          if (e.key === "ArrowDown") {
            e.preventDefault();
            if (!open) setOpen(true);
            else stepSelection(1);
          } else if (e.key === "ArrowUp") {
            e.preventDefault();
            if (!open) setOpen(true);
            else stepSelection(-1);
          }
        }}
      >
        <span className={`custom-select-value${!current ? " placeholder" : ""}`}>
          {current?.label ?? placeholder ?? ""}
        </span>
        <PhIcon name="caret-down" size={14} className="custom-select-caret" />
      </button>
      {open && (
        <div id={listId} role="listbox" className="custom-select-menu">
          {options.map((o) => (
            <button
              key={o.value}
              type="button"
              role="option"
              aria-selected={o.value === value}
              disabled={o.disabled}
              className={`custom-select-option${o.value === value ? " selected" : ""}`}
              onClick={() => {
                if (o.disabled) return;
                onChange(o.value);
                setOpen(false);
              }}
            >
              <span className="custom-select-option-label">{o.label}</span>
              {o.value === value && <PhIcon name="check" size={14} color="var(--accent)" />}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
