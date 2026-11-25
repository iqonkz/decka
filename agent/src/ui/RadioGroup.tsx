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
  return (
    <div className={`radio-group${className ? ` ${className}` : ""}`} role="radiogroup" aria-label={ariaLabel}>
      {options.map((o) => {
        const checked = o.value === value;
        return (
          <button
            key={o.value}
            type="button"
            role="radio"
            aria-checked={checked}
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
