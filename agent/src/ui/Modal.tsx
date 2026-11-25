import { useEffect, type ReactNode } from "react";

export interface ModalProps {
  title: string;
  subtitle?: string;
  onClose: () => void;
  children: ReactNode;
  footer?: ReactNode;
  className?: string;
  /** Ignore Esc and backdrop clicks (forms where an accidental close would lose input). */
  persistent?: boolean;
}

/** Shared modal window matching DESIGN.md §4.15 (radius 16, padding 24, 18/600 title, right-aligned footer). */
export default function Modal({ title, subtitle, onClose, children, footer, className, persistent }: ModalProps) {
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape" && !persistent) onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose, persistent]);

  return (
    <div className="modal-backdrop" onPointerDown={persistent ? undefined : onClose}>
      <div
        className={`modal-card${className ? ` ${className}` : ""}`}
        role="dialog"
        aria-modal="true"
        aria-label={title}
        onPointerDown={(e) => e.stopPropagation()}
      >
        <div className="modal-head">
          <h2 className="modal-title">{title}</h2>
          {subtitle && <p className="modal-subtitle">{subtitle}</p>}
        </div>
        <div className="modal-body">{children}</div>
        {footer && <div className="modal-footer">{footer}</div>}
      </div>
    </div>
  );
}
