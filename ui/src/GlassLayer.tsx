import type { WallpaperId } from "@decka/shared";

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
        {album && <img className="wallpaper-art" src={art} alt="" draggable={false} />}
      </div>
    </>
  );
}
