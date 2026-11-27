import { useEffect } from "react";
import { clearHint, useAppState } from "../store/store";

const DEFAULT_HINT_MS = 2000;

/** The name of the page that was just opened automatically or via swipe. Never takes touches. */
export default function PageHint() {
  const hint = useAppState((s) => s.hint);
  const id = hint?.id;
  const ms = hint?.ms ?? DEFAULT_HINT_MS;
  useEffect(() => {
    if (id === undefined) return;
    const timer = window.setTimeout(() => clearHint(id), ms);
    return () => window.clearTimeout(timer);
  }, [id, ms]);
  if (!hint) return null;
  return (
    <div className="page-hint" aria-live="polite">
      {hint.text}
    </div>
  );
}
