import PhIcon from "./PhIcon";

export interface CheckboxProps {
  checked: boolean;
  onChange: (checked: boolean) => void;
  disabled?: boolean;
  "aria-label"?: string;
  id?: string;
  className?: string;
}

/** Custom 18 px table checkbox matching DESIGN.md §4.15 (radius 5, --accent, check icon). */
export default function Checkbox({
  checked,
  onChange,
  disabled = false,
  "aria-label": ariaLabel,
  id,
  className,
}: CheckboxProps) {
  return (
    <button
      type="button"
      id={id}
      role="checkbox"
      aria-checked={checked}
      aria-label={ariaLabel}
      disabled={disabled}
      className={`custom-checkbox${checked ? " checked" : ""}${className ? ` ${className}` : ""}`}
      onClick={() => !disabled && onChange(!checked)}
    >
      {checked && <PhIcon name="check" size={12} weight="bold" color="#fff" />}
    </button>
  );
}
