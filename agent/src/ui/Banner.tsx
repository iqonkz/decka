import type { ReactNode } from "react";

export default function Banner({ kind, children }: { kind: "info" | "error" | "success" | "warning"; children: ReactNode }) {
  return (
    <div className={`banner banner-${kind}`} role={kind === "error" ? "alert" : "status"}>
      {children}
    </div>
  );
}
