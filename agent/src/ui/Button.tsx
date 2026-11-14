import type { ButtonHTMLAttributes } from "react";

type Variant = "primary" | "secondary" | "danger";

export default function Button({ variant = "secondary", className, ...rest }: ButtonHTMLAttributes<HTMLButtonElement> & { variant?: Variant }) {
  return <button {...rest} type={rest.type ?? "button"} className={`btn btn-${variant}${className ? ` ${className}` : ""}`} />;
}
