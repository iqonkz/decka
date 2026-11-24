import { useState, type ReactNode } from "react";
import { t } from "@decka/shared";
import Button from "./Button";

/** A destructive button that asks first: the first click shows `question` with Yes / Cancel in its place (DATA_MODEL.md 13.7). */
export default function ConfirmButton({ label, question, onConfirm, icon, disabled, title }: { label: ReactNode; question: string; onConfirm: () => void; icon?: boolean; disabled?: boolean; title?: string }) {
  const [asking, setAsking] = useState(false);
  if (!asking) {
    return (
      <Button variant="danger" className={icon ? "btn-icon" : undefined} disabled={disabled} onClick={() => setAsking(true)} title={title ?? (typeof label === "string" ? label : undefined)} aria-label={title ?? (typeof label === "string" ? label : undefined)}>
        {label}
      </Button>
    );
  }
  return (
    <span className="confirm-inline" role="alertdialog" aria-label={question}>
      <span className="muted">{question}</span>
      <Button
        variant="danger"
        onClick={() => {
          setAsking(false);
          onConfirm();
        }}
      >
        {t("voice.yes")}
      </Button>
      <Button onClick={() => setAsking(false)}>{t("common.cancel")}</Button>
    </span>
  );
}