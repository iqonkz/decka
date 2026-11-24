import type { CSSProperties } from "react";

export interface PhIconProps {
  name: string;
  size?: number;
  weight?: "regular" | "fill" | "bold" | "light" | "thin" | "duotone" | string;
  color?: string;
  style?: CSSProperties;
  className?: string;
  title?: string;
}

/** A Phosphor icon from the copy of the library that ships with the window; follows the text colour by default. */
export default function PhIcon({
  name,
  size = 20,
  weight = "regular",
  color,
  style,
  className,
  title,
}: PhIconProps) {
  // Phosphor names the files of every weight but regular with a suffix: fill/check-circle-fill.svg.
  const mask = `url("phosphor/${weight}/${weight === "regular" ? name : `${name}-${weight}`}.svg")`;
  return (
    <span
      aria-hidden={title ? undefined : "true"}
      title={title}
      className={`ph-icon${className ? ` ${className}` : ""}`}
      style={{
        display: "inline-block",
        flex: "none",
        width: size,
        height: size,
        backgroundColor: color ?? "currentColor",
        WebkitMaskImage: mask,
        maskImage: mask,
        WebkitMaskSize: "contain",
        maskSize: "contain",
        WebkitMaskRepeat: "no-repeat",
        maskRepeat: "no-repeat",
        WebkitMaskPosition: "center",
        maskPosition: "center",
        ...style,
      }}
    />
  );
}
