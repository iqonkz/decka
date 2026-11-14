import { useEffect, useRef, useState, type RefObject } from "react";
import { keySize } from "./look";

/** Side of the key in px for the cell behind `ref`; follows the cell size (ResizeObserver). */
export function useKeySize(): [RefObject<HTMLDivElement | null>, number] {
  const ref = useRef<HTMLDivElement>(null);
  const [size, setSize] = useState(48);
  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    const measure = () => setSize(keySize(el.clientWidth, el.clientHeight));
    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(el);
    return () => observer.disconnect();
  }, []);
  return [ref, size];
}
