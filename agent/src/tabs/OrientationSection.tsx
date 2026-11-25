import { useEffect, useMemo, useRef, useState } from "react";
import { rotateLayout, t, tn, type LayoutFile, type Orientation } from "@decka/shared";
import { EMPTY_DATA, GlassLayer, Grid, resolveTheme } from "@decka/ui";
import { call } from "../ipc";
import Banner from "../ui/Banner";
import Button from "../ui/Button";
import Modal from "../ui/Modal";
import PhIcon from "../ui/PhIcon";
import PhoneFrame from "./PhoneFrame";

const DATA = { ...EMPTY_DATA, online: true, preview: true };
const NOOP = { onPress: () => Promise.resolve(), onPressError: () => {}, onNavigate: () => {}, onDismiss: () => {} };

/** A silhouette of a phone, wide or tall (the two cards of `Orientation`). */
function Silhouette({ orientation }: { orientation: Orientation }) {
  const portrait = orientation === "portrait";
  return (
    <svg className="phone-silhouette" width={portrait ? 28 : 48} height={portrait ? 48 : 28} viewBox={portrait ? "0 0 28 48" : "0 0 48 28"} aria-hidden="true">
      <rect x="1.5" y="1.5" width={portrait ? 25 : 45} height={portrait ? 45 : 25} rx="4" fill="none" stroke="currentColor" strokeWidth="2" />
      <rect x={portrait ? 10 : 36} y={portrait ? 4 : 10} width={portrait ? 8 : 2} height={portrait ? 2 : 8} rx="1" fill="currentColor" />
    </svg>
  );
}

/** The first page of the turned layout in a small phone frame (what the deck will look like). */
function Preview({ layout }: { layout: LayoutFile }) {
  const resolved = resolveTheme(layout.display);
  const page = layout.pages[0]!;
  const noop = useMemo(() => NOOP, []);
  return (
    <PhoneFrame availableWidth={260} maxHeight={300} display={layout.display}>
      {resolved.theme === "glass" && <GlassLayer wallpaper={resolved.wallpaper} art={undefined} />}
      <div style={{ position: "absolute", inset: 0 }}>
        <Grid page={page} grid={layout.grid} data={DATA} {...noop} />
      </div>
    </PhoneFrame>
  );
}

interface Props {
  /** The orientation the deck has now. */
  orientation: Orientation;
  /** The layout was turned (or the turn was undone): the tab reloads its display settings from it. */
  onChanged: (layout: LayoutFile) => void;
}

/**
 * `Look → Orientation` (DESIGN.md 4.6a, DATA_MODEL.md 3.7): two cards with a phone silhouette. Choosing the other one shows
 * the turned deck and how many items go to new pages; `Rotate` saves it and `Ctrl+Z` takes the whole turn back.
 */
export default function OrientationSection({ orientation, onChanged }: Props) {
  const [pending, setPending] = useState<{ layout: LayoutFile; moved: number; to: Orientation } | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  // The layout as it was before the last turn: `Ctrl+Z` writes it back.
  const undoRef = useRef<LayoutFile | null>(null);

  useEffect(() => {
    const onKey = async (e: KeyboardEvent) => {
      if (!(e.ctrlKey || e.metaKey) || e.key.toLowerCase() !== "z" || e.shiftKey || !undoRef.current) return;
      if (e.target instanceof HTMLElement && /^(INPUT|TEXTAREA|SELECT)$/.test(e.target.tagName)) return;
      e.preventDefault();
      const before = undoRef.current;
      undoRef.current = null;
      const result = await call("save_layout", { layout: before });
      if (result.ok) {
        onChanged(before);
        setMessage(t("orientation.undone"));
      } else setError(result.issues[0]?.message ?? "");
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onChanged]);

  async function choose(to: Orientation) {
    if (to === orientation) return;
    setError(null);
    setMessage(null);
    const fresh = await call("get_layout");
    const turned = rotateLayout(fresh, to);
    if (!turned.ok) return setError(turned.error);
    setPending({ layout: turned.layout, moved: turned.moved, to });
  }

  async function confirm() {
    if (!pending) return;
    const before = await call("get_layout");
    const result = await call("save_layout", { layout: pending.layout });
    if (result.ok) {
      undoRef.current = before;
      onChanged(pending.layout);
      setMessage(t("orientation.rotated"));
    } else setError(result.issues[0]?.message ?? "");
    setPending(null);
  }

  return (
    <>
      <div className="orientation-cards" role="radiogroup" aria-label={t("orientation.title")}>
        {(["landscape", "portrait"] as const).map((o) => (
          <button key={o} type="button" role="radio" aria-checked={orientation === o} className={orientation === o ? "style-card selected" : "style-card"} onClick={() => void choose(o)}>
            {orientation === o && (
              <span className="selection-badge">
                <PhIcon name="check-circle" size={20} weight="fill" color="var(--accent)" />
              </span>
            )}
            <Silhouette orientation={o} />
            <strong>{t(`orientation.${o}`)}</strong>
            <small className="muted">{t(`orientation.${o}_hint`)}</small>
          </button>
        ))}
      </div>
      {error && <Banner kind="error">{error}</Banner>}
      {message && <Banner kind="success">{message}</Banner>}
      {pending && (
        <Modal
          title={t(`orientation.turn_to_${pending.to}`)}
          onClose={() => setPending(null)}
          footer={
            <>
              <Button onClick={() => setPending(null)}>{t("common.cancel")}</Button>
              <Button variant="primary" onClick={() => void confirm()}>
                {t("orientation.rotate")}
              </Button>
            </>
          }
        >
          <Preview layout={pending.layout} />
          <p className="muted" style={{ margin: 0 }}>{pending.moved > 0 ? tn("orientation.moved", pending.moved) : t("orientation.nothing_moved")}</p>
        </Modal>
      )}
    </>
  );
}
