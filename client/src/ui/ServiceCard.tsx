import type { ReactNode } from "react";
import PhIcon, { type PhIconName } from "./PhIcon";

/**
 * Centered service card (DESIGN.md 4.16): <= 420 px wide, 88 px accent-15% circle with 48 px Phosphor icon,
 * 22/600 title, 15 px 70% body, and full-width 48 px pill actions.
 */
export default function ServiceCard({
  icon,
  title,
  text,
  children,
  backdrop = false,
  onBackdropPress,
  role = "region",
  asForm = false,
  onSubmit,
}: {
  icon: PhIconName;
  title: string;
  text?: string;
  children?: ReactNode;
  backdrop?: boolean;
  onBackdropPress?: () => void;
  role?: "dialog" | "region";
  asForm?: boolean;
  onSubmit?: (e: React.FormEvent) => void;
}) {
  const inner = (
    <>
      <div className="service-hero">
        <PhIcon name={icon} size={56} />
      </div>
      <h1 className="service-title">{title}</h1>
      {text && <p className="service-text">{text}</p>}
      {children}
    </>
  );

  const card = asForm ? (
    <form className="service-card" onSubmit={onSubmit} onPointerDown={(e) => e.stopPropagation()}>
      {inner}
    </form>
  ) : (
    <div className="service-card" role={role} aria-label={title} onPointerDown={(e) => e.stopPropagation()}>
      {inner}
    </div>
  );

  return (
    <div className={backdrop ? "service-wrap modal" : "service-wrap"} onPointerDown={onBackdropPress}>
      {card}
    </div>
  );
}
