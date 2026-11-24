import type { ReactNode } from "react";

export interface SegmentOption<T extends string = string> {
  value: T;
  label: ReactNode;
  title?: string;
  disabled?: boolean;
}

export interface SegmentedProps<T extends string = string> {
  options: Array<SegmentOption<T> | T>;
  value: T;
  onChange: (value: T) => void;
  className?: string;
  "aria-label"?: string;
}

export default function Segmented<T extends string = string>({
  options,
  value,
  onChange,
  className,
  "aria-label": ariaLabel,
}: SegmentedProps<T>) {
  return (
    <div
      role="radiogroup"
      aria-label={ariaLabel}
      className={`segmented-track${className ? ` ${className}` : ""}`}
    >
      {options.map((opt) => {
        const item: SegmentOption<T> = typeof opt === "string" ? { value: opt as T, label: opt } : opt;
        const active = item.value === value;
        return (
          <button
            key={item.value}
            type="button"
            role="radio"
            aria-checked={active}
            title={item.title}
            disabled={item.disabled}
            className={`segmented-item${active ? " active" : ""}`}
            onClick={() => onChange(item.value)}
          >
            {item.label}
          </button>
        );
      })}
    </div>
  );
}
