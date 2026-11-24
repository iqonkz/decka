import type { CSSProperties } from "react";

/** Skeleton loading panel rectangle (DESIGN.md 4.16): 30% opacity surface with 1.2s shimmer (static in reduced mode). */
export default function Skeleton({
  width = "100%",
  height = "100%",
  radius,
  className,
  style,
}: {
  width?: number | string;
  height?: number | string;
  radius?: number | string;
  className?: string;
  style?: CSSProperties;
}) {
  return (
    <span
      aria-hidden="true"
      className={className ? `skeleton ${className}` : "skeleton"}
      style={{
        width,
        height,
        borderRadius: radius,
        ...style,
      }}
    />
  );
}
