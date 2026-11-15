import { useEffect, useRef } from "react";
import type { WallpaperId } from "@decka/shared";

/** Side of the canvas the cover is drawn into: stretched to the screen it is a blur without `filter: blur()` (D-2). */
const ART_SIZE = 24;

/** The cover, drawn once per `art` into a small canvas, darkened and saturated (DESIGN.md 1.5). */
function AlbumArt({ art }: { art: string }) {
  const ref = useRef<HTMLCanvasElement>(null);
  useEffect(() => {
    const canvas = ref.current;
    const ctx = canvas?.getContext("2d");
    if (!canvas || !ctx) return;
    let cancelled = false;
    const img = new Image();
    img.onload = () => {
      if (cancelled) return;
      const hasFilter: boolean = typeof ctx.filter === "string";
      if (hasFilter) ctx.filter = "brightness(0.55) saturate(140%)";
      // `cover`: the central square of the cover.
      const s = Math.min(img.naturalWidth, img.naturalHeight);
      ctx.drawImage(img, (img.naturalWidth - s) / 2, (img.naturalHeight - s) / 2, s, s, 0, 0, ART_SIZE, ART_SIZE);
      if (!hasFilter) {
        ctx.fillStyle = "rgba(0,0,0,0.45)"; // no canvas filters: darken by hand
        ctx.fillRect(0, 0, ART_SIZE, ART_SIZE);
      }
    };
    img.src = art;
    return () => {
      cancelled = true;
    };
  }, [art]);
  return <canvas ref={ref} className="wallpaper-art" width={ART_SIZE} height={ART_SIZE} />;
}

/**
 * What the Glass theme needs behind the keys (DESIGN.md 1.4, 1.5): the SVG filter `#decka-glass` for the
 * refraction layer and the wallpaper. `art` is the cover of the current track (wallpaper `album`).
 */
export default function GlassLayer({ wallpaper, art }: { wallpaper: WallpaperId; art: string | undefined }) {
  const album = wallpaper === "album" && art !== undefined;
  // `album` without a cover is `aurora`.
  const id = wallpaper === "album" && !album ? "aurora" : wallpaper;
  return (
    <>
      <svg width="0" height="0" style={{ position: "absolute" }} aria-hidden="true">
        <filter id="decka-glass" x="-10%" y="-10%" width="120%" height="120%" colorInterpolationFilters="sRGB">
          <feTurbulence type="fractalNoise" baseFrequency="0.008" numOctaves={2} seed={3} result="noise" />
          <feGaussianBlur in="noise" stdDeviation={2} result="soft" />
          <feDisplacementMap in="SourceGraphic" in2="soft" scale={26} xChannelSelector="R" yChannelSelector="G" />
        </filter>
      </svg>
      <div className="wallpaper" data-wallpaper={id} aria-hidden="true">
        {album && art !== undefined && <AlbumArt art={art} />}
      </div>
    </>
  );
}
