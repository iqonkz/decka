export interface RadioOption<T extends string = string> {
  value: T;
  label: string;
}

export interface RadioGroupProps<T extends string = string> {
  value: T;
  options: readonly RadioOption<T>[];
  onChange: (value: T) => void;
  "aria-label"?: string;
  className?: string;
}

/** Custom radio pill group matching DESIGN §4.14 (replaces native input[type="radio"]). */
export default function RadioGroup<T extends string = string>({
  value,
  options,
  onChange,
  "aria-label": ariaLabel,
  className,
}: RadioGroupProps<T>) {
  // Arrow keys move the choice (and the focus) like native radios; Tab enters the group on the checked one only.
  const onKey = (e: React.KeyboardEvent<HTMLDivElement>) => {
    const step = e.key === "ArrowRight" || e.key === "ArrowDown" ? 1 : e.key === "ArrowLeft" || e.key === "ArrowUp" ? -1 : 0;
    if (!step || options.length === 0) return;
    e.preventDefault();
    const at = Math.max(0, options.findIndex((o) => o.value === value));
    const next = options[(at + step + options.length) % options.length] as RadioOption<T>;
    onChange(next.value);
    const buttons = e.currentTarget.querySelectorAll<HTMLButtonElement>('[role="radio"]');
    buttons[options.indexOf(next)]?.focus();
  };
  const anyChecked = options.some((o) => o.value === value);
  return (
    <div className={`radio-group${className ? ` ${className}` : ""}`} role="radiogroup" aria-label={ariaLabel} onKeyDown={onKey}>
      {options.map((o, i) => {
        const checked = o.value === value;
        return (
          <button
            key={o.value}
            type="button"
            role="radio"
            aria-checked={checked}
            tabIndex={checked || (!anyChecked && i === 0) ? 0 : -1}
            className={`radio-pill${checked ? " checked" : ""}`}
            onClick={() => onChange(o.value)}
          >
            <span className="radio-dot" aria-hidden="true" />
            <span>{o.label}</span>
          </button>
        );
      })}
    </div>
  );
}
