import { useEffect, useLayoutEffect, useRef, useState } from "react";
import PhIcon from "../ui/PhIcon";

export interface MenuItem {
  label: string;
  /** Phosphor icon name. */
  icon?: string;
  /** Shortcut hint on the right. */
  hint?: string;
  checked?: boolean;
  disabled?: boolean;
  danger?: boolean;
  onClick?: () => void;
  /** A submenu: opens beside the item. */
  submenu?: MenuItem[];
  /** A line is drawn above this item. */
  separator?: boolean;
  /** A colour swatch instead of an icon. */
  swatch?: string;
}

interface Props {
  x: number;
  y: number;
  items: MenuItem[];
  onClose: () => void;
}

/**
 * Our own context menu (DESIGN.md 4.6): opens at the cursor, moves inside the window, closes on `Esc`, an outside click or a
 * choice; arrows and `Enter` work. The native WebView2 menu is suppressed for the whole window (see `suppressNativeMenu`).
 */
export default function ContextMenu({ x, y, items, onClose }: Props) {
  const ref = useRef<HTMLDivElement>(null);
  const [pos, setPos] = useState({ left: x, top: y });
  const [active, setActive] = useState(0);
  const [openSub, setOpenSub] = useState<number | null>(null);

  useLayoutEffect(() => {
    const box = ref.current?.getBoundingClientRect();
    if (!box) return;
    setPos({ left: Math.max(4, Math.min(x, window.innerWidth - box.width - 4)), top: Math.max(4, Math.min(y, window.innerHeight - box.height - 4)) });
  }, [x, y, items]);

  useEffect(() => {
    const enabled = (i: number) => items[i] && items[i]!.disabled !== true;
    const step = (from: number, dir: 1 | -1) => {
      for (let n = 1; n <= items.length; n++) {
        const i = (from + dir * n + items.length * 2) % items.length;
        if (enabled(i)) return i;
      }
      return from;
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        e.stopPropagation();
        if (openSub !== null) setOpenSub(null);
        else onClose();
      } else if (e.key === "ArrowDown") {
        e.preventDefault();
        if (openSub === null) setActive((i) => step(i, 1));
      } else if (e.key === "ArrowUp") {
        e.preventDefault();
        if (openSub === null) setActive((i) => step(i, -1));
      } else if (e.key === "ArrowRight") {
        if (items[active]?.submenu) {
          e.preventDefault();
          setOpenSub(active);
        }
      } else if (e.key === "ArrowLeft") {
        setOpenSub(null);
      } else if (e.key === "Enter" && openSub === null) {
        const item = items[active];
        if (!item || item.disabled) return;
        e.preventDefault();
        if (item.submenu) setOpenSub(active);
        else {
          onClose();
          item.onClick?.();
        }
      }
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [items, active, openSub, onClose]);

  return (
    <>
      <div className="menu-backdrop" onPointerDown={onClose} onContextMenu={(e) => { e.preventDefault(); onClose(); }} />
      <div className="context-menu" ref={ref} role="menu" style={{ left: pos.left, top: pos.top }}>
        {items.map((item, i) => (
          <div key={`${item.label}${i}`} className="menu-entry">
            {item.separator && <div className="menu-sep" />}
            <button
              type="button"
              role={item.checked !== undefined ? "menuitemcheckbox" : "menuitem"}
              aria-checked={item.checked}
              disabled={item.disabled}
              className={`menu-item${i === active ? " active" : ""}${item.danger ? " danger" : ""}`}
              onMouseEnter={() => { setActive(i); setOpenSub(item.submenu ? i : null); }}
              onClick={() => {
                if (item.submenu) return setOpenSub(i);
                onClose();
                item.onClick?.();
              }}
            >
              {item.swatch ? <span className="menu-swatch" style={{ background: item.swatch }} /> : item.icon ? <PhIcon name={item.icon} size={16} /> : <span className="menu-gap" />}
              <span className="menu-label">{item.label}</span>
              {item.checked !== undefined && <span className="menu-check">{item.checked ? "✓" : ""}</span>}
              {item.hint && <small className="muted">{item.hint}</small>}
              {item.submenu && <PhIcon name="caret-right" size={14} />}
            </button>
            {item.submenu && openSub === i && (
              <div className="context-menu submenu" role="menu">
                {item.submenu.map((sub, n) => (
                  <button
                    key={`${sub.label}${n}`}
                    type="button"
                    role="menuitem"
                    disabled={sub.disabled}
                    className={`menu-item${sub.danger ? " danger" : ""}`}
                    onClick={() => {
                      onClose();
                      sub.onClick?.();
                    }}
                  >
                    {sub.swatch ? <span className="menu-swatch" style={{ background: sub.swatch }} /> : <span className="menu-gap" />}
                    <span className="menu-label">{sub.label}</span>
                    {sub.checked && <span className="menu-check">✓</span>}
                  </button>
                ))}
              </div>
            )}
          </div>
        ))}
      </div>
    </>
  );
}

/** The window's own menus replace the WebView2 menu (`Back`, `Refresh`, `Inspect`); text fields keep theirs for copy and paste. */
export function suppressNativeMenu(): () => void {
  const handler = (e: MouseEvent) => {
    const t = e.target;
    if (t instanceof HTMLElement && (t.closest("input, textarea") || t.isContentEditable)) return;
    e.preventDefault();
  };
  document.addEventListener("contextmenu", handler);
  return () => document.removeEventListener("contextmenu", handler);
}