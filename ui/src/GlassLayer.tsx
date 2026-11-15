import { useEffect, useRef, useState } from "react";
import type { WallpaperId } from "@decka/shared";
import { bitmapSize, canvasToObjectUrl, renderFrost, renderWallpaper } from "./wallpaper";

/** Custom property on the root that holds the frosted picture for the keys in `reduced` (glass.css). */
export const FROST_IMAGE_VAR = "--frost-image";

type Status = "pending" | "ready" | "failed";

/**
 * What the Glass theme needs behind the keys (DESIGN.md 1.4, 1.5): the SVG filter `#decka-glass` for the
 * refraction layer and the wallpaper. `art` is the cover of the current track (wallpaper `album`). The wallpaper is
 * drawn once into a canvas of a quarter of the screen, already blurred and dimmed; if the canvas cannot be used the CSS
 * gradients stay. `frost` (the `reduced` mode) also prepares the blurred copy the keys show instead of `backdrop-filter`.
 */
export default function GlassLayer({ wallpaper, art, frost = false }: { wallpaper: WallpaperId; art: string | undefined; frost?: boolean }) {
  const album = wallpaper === "album" && art !== undefined;
  // `album` without a cover is `aurora`.
  const id = wallpaper === "album" && !album ? "aurora" : wallpaper;
  const layer = useRef<HTMLDivElement>(null);
  const [status, setStatus] = useState<Status>("pending");
  const [url, setUrl] = useState<string | null>(null);
  const current = useRef<string | null>(null);
  const frostUrl = useRef<string | null>(null);

  useEffect(() => {
    const el = layer.current;
    if (!el) return;
    let cancelled = false;
    void (async () => {
      const size = bitmapSize(el.offsetWidth || window.innerWidth, el.offsetHeight || window.innerHeight, window.devicePixelRatio || 1);
      let base: HTMLCanvasElement | null = null;
      try {
        base = await renderWallpaper(id, album ? art : undefined, size);
      } catch {
        base = null;
      }
      const next = base ? await canvasToObjectUrl(base) : null;
      if (cancelled) {
        if (next) URL.revokeObjectURL(next);
        return;
      }
      if (!base || !next) {
        setStatus("failed");
        return;
      }
      // Keep the old picture on screen until the new one is ready, then drop it.
      const old = current.current;
      current.current = next;
      setUrl(next);
      setStatus("ready");
      if (old) URL.revokeObjectURL(old);

      const root = document.documentElement;
      const oldFrost = frostUrl.current;
      let nextFrost: string | null = null;
      if (frost) {
        const f = renderFrost(base);
        nextFrost = f ? await canvasToObjectUrl(f) : null;
        if (cancelled) {
          if (nextFrost) URL.revokeObjectURL(nextFrost);
          return;
        }
      }
      frostUrl.current = nextFrost;
      if (nextFrost) root.style.setProperty(FROST_IMAGE_VAR, `url("${nextFrost}")`);
      else root.style.removeProperty(FROST_IMAGE_VAR);
      if (oldFrost) URL.revokeObjectURL(oldFrost);
    })();
    return () => {
      cancelled = true;
    };
  }, [id, album, art, frost]);

  // Unmount: free the pictures and the property.
  useEffect(
    () => () => {
      if (current.current) URL.revokeObjectURL(current.current);
      if (frostUrl.current) URL.revokeObjectURL(frostUrl.current);
      current.current = frostUrl.current = null;
      document.documentElement.style.removeProperty(FROST_IMAGE_VAR);
    },
    [],
  );

  return (
    <>
      <svg width="0" height="0" style={{ position: "absolute" }} aria-hidden="true">
        <filter id="decka-glass" x="-10%" y="-10%" width="120%" height="120%" colorInterpolationFilters="sRGB">
          <feTurbulence type="fractalNoise" baseFrequency="0.008" numOctaves={2} seed={3} result="noise" />
          <feGaussianBlur in="noise" stdDeviation={2} result="soft" />
          <feDisplacementMap in="SourceGraphic" in2="soft" scale={26} xChannelSelector="R" yChannelSelector="G" />
        </filter>
      </svg>
      <div
        ref={layer}
        className="wallpaper"
        data-wallpaper={id}
        data-status={status}
        style={url && status === "ready" ? { backgroundImage: `url("${url}")` } : undefined}
        aria-hidden="true"
      />
    </>
  );
}
