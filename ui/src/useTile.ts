import { useEffect, useState } from "react";
import { isTileCorners } from "./faceLayout";

const cache = new Map<string, boolean>();

/** Once per picture (by its data URL): are the four corner pixels opaque? Reads them through a canvas. */
function corners(url: string): Promise<boolean> {
  return new Promise((resolve) => {
    const img = new Image();
    img.onload = () => {
      try {
        const w = img.naturalWidth;
        const h = img.naturalHeight;
        const canvas = document.createElement("canvas");
        canvas.width = w;
        canvas.height = h;
        const ctx = canvas.getContext("2d", { willReadFrequently: true });
        if (!ctx || w < 2 || h < 2) return resolve(false);
        ctx.drawImage(img, 0, 0);
        const alpha = (x: number, y: number) => ctx.getImageData(x, y, 1, 1).data[3] ?? 0;
        resolve(isTileCorners([alpha(0, 0), alpha(w - 1, 0), alpha(0, h - 1), alpha(w - 1, h - 1)]));
      } catch {
        resolve(false); // a canvas that cannot be read: treat as a transparent picture
      }
    };
    img.onerror = () => resolve(false);
    img.src = url;
  });
}

/** `true` when `url` is a picture with opaque corners (a tile); `false` until known and for anything else. */
export function useTile(url: string | undefined): boolean {
  const [tile, setTile] = useState(() => (url !== undefined && cache.get(url)) || false);
  useEffect(() => {
    if (!url || url.startsWith("data:image/svg+xml")) return setTile(false);
    const known = cache.get(url);
    if (known !== undefined) return setTile(known);
    let live = true;
    void corners(url).then((value) => {
      cache.set(url, value);
      if (live) setTile(value);
    });
    return () => {
      live = false;
    };
  }, [url]);
  return tile;
}