import { useEffect, useRef, useState, type PointerEvent as ReactPointerEvent } from "react";
import { t } from "@decka/shared";
import PhIcon from "./PhIcon";
import Button from "./Button";
import { clamp, hexToHsv, hsvToHex, normalizeHex, type HSV } from "./colorUtils";

export const ACCENTS = [
  "#7c5cff",
  "#3b82f6",
  "#06b6d4",
  "#16a34a",
  "#eab308",
  "#f97316",
  "#ef4444",
  "#ec4899",
] as const;

export interface ColorPickerProps {
  value: string;
  onChange: (hex: string) => void;
  compact?: boolean;
  "aria-label"?: string;
}

export default function ColorPicker({ value, onChange, compact = false, "aria-label": ariaLabel }: ColorPickerProps) {
  const currentHex = normalizeHex(value);
  const isCustom = !ACCENTS.some((c) => c.toLowerCase() === currentHex.toLowerCase());
  const [open, setOpen] = useState(false);
  const [hsv, setHsv] = useState<HSV>(() => hexToHsv(currentHex));
  const [hexInput, setHexInput] = useState(currentHex.toUpperCase());
  const popoverRef = useRef<HTMLDivElement>(null);
  const satValRef = useRef<HTMLDivElement>(null);
  const hueRef = useRef<HTMLDivElement>(null);

  // Sync internal state if external value changes while popover is closed
  useEffect(() => {
    if (!open) {
      const h = hexToHsv(currentHex);
      setHsv(h);
      setHexInput(currentHex.toUpperCase());
    }
  }, [currentHex, open]);

  // Close popover on outside click
  useEffect(() => {
    if (!open) return;
    const onPointerDown = (e: PointerEvent) => {
      if (popoverRef.current && !popoverRef.current.contains(e.target as Node)) {
        setOpen(false);
      }
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

  const updateHsv = (patch: Partial<HSV>) => {
    const next: HSV = { ...hsv, ...patch };
    setHsv(next);
    const hex = hsvToHex(next.h, next.s, next.v);
    setHexInput(hex.toUpperCase());
    onChange(hex);
  };

  const handleSatValPointer = (e: ReactPointerEvent<HTMLDivElement> | PointerEvent) => {
    if (!satValRef.current) return;
    const rect = satValRef.current.getBoundingClientRect();
    const x = clamp((e.clientX - rect.left) / rect.width, 0, 1);
    const y = clamp((e.clientY - rect.top) / rect.height, 0, 1);
    updateHsv({ s: x, v: 1 - y });
  };

  const onSatValDown = (e: ReactPointerEvent<HTMLDivElement>) => {
    e.preventDefault();
    handleSatValPointer(e);
    const onMove = (ev: PointerEvent) => handleSatValPointer(ev);
    const onUp = () => {
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", onUp);
    };
    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", onUp);
  };

  const handleHuePointer = (e: ReactPointerEvent<HTMLDivElement> | PointerEvent) => {
    if (!hueRef.current) return;
    const rect = hueRef.current.getBoundingClientRect();
    const x = clamp((e.clientX - rect.left) / rect.width, 0, 1);
    updateHsv({ h: Math.round(x * 360) });
  };

  const onHueDown = (e: ReactPointerEvent<HTMLDivElement>) => {
    e.preventDefault();
    handleHuePointer(e);
    const onMove = (ev: PointerEvent) => handleHuePointer(ev);
    const onUp = () => {
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", onUp);
    };
    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", onUp);
  };

  const handleHexInputChange = (str: string) => {
    setHexInput(str);
    const clean = str.trim();
    if (/^#[0-9a-fA-F]{6}$/.test(clean)) {
      const nextHsv = hexToHsv(clean);
      setHsv(nextHsv);
      onChange(clean.toLowerCase());
    }
  };

  return (
    <div className="color-picker-wrap" ref={popoverRef}>
      {compact ? (
        <button
          type="button"
          className="color-swatch-circle selected"
          style={{ backgroundColor: currentHex }}
          aria-label={ariaLabel ?? t("look.custom_accent")}
          onClick={() => setOpen(!open)}
        />
      ) : (
        <div className="swatches-row">
          {ACCENTS.map((c) => {
            const isSelected = currentHex.toLowerCase() === c.toLowerCase();
            return (
              <button
                key={c}
                type="button"
                className={`color-swatch-circle ${isSelected ? "selected" : ""}`}
                style={{ backgroundColor: c }}
                aria-label={t("look.accent_swatch", { c })}
                onClick={() => {
                  setOpen(false);
                  onChange(c);
                }}
              >
                {isSelected && <PhIcon name="check" size={14} color="#fff" weight="bold" />}
              </button>
            );
          })}

          {/* 9th Swatch: Custom Color */}
          <button
            type="button"
            className={`color-swatch-circle custom ${isCustom ? "selected custom-active" : ""}`}
            style={isCustom ? { backgroundColor: currentHex } : undefined}
            aria-label={t("look.custom_accent")}
            onClick={() => setOpen(!open)}
          >
            {isCustom ? (
              <PhIcon name="check" size={14} color="#fff" weight="bold" />
            ) : (
              <PhIcon name="plus" size={14} color="#fff" weight="bold" />
            )}
          </button>
        </div>
      )}

      {open && (
        <div className="color-picker-popover" role="dialog" aria-label={t("look.custom_accent")}>
          {/* Saturation / Brightness 2D Square */}
          <div
            ref={satValRef}
            className="color-picker-satval"
            style={{ backgroundColor: `hsl(${hsv.h}, 100%, 50%)` }}
            onPointerDown={onSatValDown}
          >
            <div className="color-picker-satval-white" />
            <div className="color-picker-satval-black" />
            <div
              className="color-picker-satval-thumb"
              style={{
                left: `${hsv.s * 100}%`,
                top: `${(1 - hsv.v) * 100}%`,
                backgroundColor: currentHex,
              }}
            />
          </div>

          {/* Hue Slider */}
          <div ref={hueRef} className="color-picker-hue" onPointerDown={onHueDown}>
            <div
              className="color-picker-hue-thumb"
              style={{
                left: `${(hsv.h / 360) * 100}%`,
              }}
            />
          </div>

          {/* HEX Input and Done Button */}
          <div className="color-picker-actions">
            <input
              type="text"
              maxLength={7}
              className="color-picker-hex-input"
              value={hexInput}
              aria-label={t("look.accent_hex")}
              onChange={(e) => handleHexInputChange(e.target.value)}
              onBlur={() => setHexInput(currentHex.toUpperCase())}
            />
            <Button variant="primary" onClick={() => setOpen(false)}>
              {t("common.done")}
            </Button>
          </div>
        </div>
      )}
    </div>
  );
}
