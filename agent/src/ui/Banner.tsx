import type { ReactNode } from "react";
import PhIcon from "./PhIcon";

export default function Banner({ kind, children }: { kind: "info" | "error" | "success" | "warning"; children: ReactNode }) {
  return (
    <div className={`banner banner-${kind}`} role={kind === "error" ? "alert" : "status"}>
      {kind === "warning" && <PhIcon name="warning" size={18} color="var(--warn)" className="banner-icon" />}
      <div className="banner-body">{children}</div>
    </div>
  );
}
