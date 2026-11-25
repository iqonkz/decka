import { useState, type CSSProperties, type ReactNode } from "react";
import PhIcon from "./PhIcon";

export interface DisclosureProps {
  title: ReactNode;
  children: ReactNode;
  defaultOpen?: boolean;
  className?: string;
  style?: CSSProperties;
}

/** Custom disclosure row replacing native <details> (DESIGN.md §4.15). */
export default function Disclosure({
  title,
  children,
  defaultOpen = false,
  className,
  style,
}: DisclosureProps) {
  const [open, setOpen] = useState(defaultOpen);
  return (
    <div className={`disclosure${open ? " open" : ""}${className ? ` ${className}` : ""}`} style={style}>
      <button
        type="button"
        className="disclosure-trigger"
        aria-expanded={open}
        onClick={() => setOpen((v) => !v)}
      >
        <PhIcon name="caret-right" size={16} className={`disclosure-caret${open ? " open" : ""}`} />
        <span className="disclosure-title">{title}</span>
      </button>
      {open && <div className="disclosure-body">{children}</div>}
    </div>
  );
}
