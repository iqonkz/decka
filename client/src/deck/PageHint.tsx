import { useEffect } from "react";
import { clearHint, useAppState } from "../store/store";

const HINT_MS = 2000;

/** The name of the page that was just opened automatically (DATA_MODEL.md 6.1). Never takes touches. */
export default function PageHint() {
  const hint = useAppState((s) => s.hint);
  const id = hint?.id;
  useEffect(() => {
    if (id === undefined) return;
    const timer = window.setTimeout(() => clearHint(id), HINT_MS);
    return () => window.clearTimeout(timer);
  }, [id]);
  if (!hint) return null;
  return (
    <div className="page-hint" aria-live="polite">
      {hint.text}
    </div>
  );
}
