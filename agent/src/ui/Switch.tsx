import type { KeyboardEvent } from "react";

export function handleSwitchKeyDown(
  e: { key: string; preventDefault: () => void },
  checked: boolean,
  onChange: (checked: boolean) => void,
  disabled?: boolean,
): boolean {
  if (disabled) return false;
  if (e.key === " " || e.key === "Spacebar" || e.key === "Enter") {
    e.preventDefault();
    onChange(!checked);
    return true;
  }
  return false;
}

export interface SwitchProps {
  id?: string;
  checked: boolean;
  onChange: (checked: boolean) => void;
  disabled?: boolean;
  label?: string;
  "aria-label"?: string;
  className?: string;
  title?: string;
}

export default function Switch({
  id,
  checked,
  onChange,
  disabled = false,
  label,
  "aria-label": ariaLabel,
  className,
  title,
}: SwitchProps) {
  return (
    <button
      type="button"
      id={id}
      role="switch"
      aria-checked={checked}
      aria-label={ariaLabel || label}
      title={title}
      disabled={disabled}
      className={`settings-switch${checked ? " checked" : ""}${className ? ` ${className}` : ""}`}
      onClick={() => !disabled && onChange(!checked)}
      onKeyDown={(e: KeyboardEvent<HTMLButtonElement>) =>
        handleSwitchKeyDown(e, checked, onChange, disabled)
      }
    >
      <span className="settings-switch-knob" aria-hidden="true" />
    </button>
  );
}
