import { useEffect, useRef, useState, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { t, type DisplaySettings } from "@decka/shared";
import { applyTheme } from "@decka/ui";

/** The viewport of the preview: the phone in landscape (CSS pixels). */
export const PHONE_W = 915;
export const PHONE_H = 412;

/** The viewport of the phone in the orientation of the deck: 915x412, or 412x915 turned (DESIGN.md 2.2). */
export function phoneView(orientation: "landscape" | "portrait" | undefined): { w: number; h: number } {
  return orientation === "portrait" ? { w: PHONE_H, h: PHONE_W } : { w: PHONE_W, h: PHONE_H };
}

/** The scale that fits a `viewW` x `viewH` screen into `availableWidth` and, when given, `maxHeight` (never enlarged). */
export function fitScale(view: { w: number; h: number }, availableWidth: number, maxHeight?: number): number {
  const byWidth = availableWidth / view.w;
  const byHeight = maxHeight === undefined ? Infinity : maxHeight / view.h;
  return Math.max(0.2, Math.min(1, byWidth, byHeight));
}

interface Props {
  /** Width in px the frame may take; it is scaled down to fit and never enlarged. */
  availableWidth: number;
  display: DisplaySettings;
  children: ReactNode;
  /** Another screen size than the phone (the key preview of the properties panel). */
  view?: { w: number; h: number };
  /** A tall (portrait) screen is also kept within this height. */
  maxHeight?: number;
}

const SRC_DOC = `<!doctype html><html><head><meta charset="utf-8"></head><body></body></html>`;

/**
 * A phone-sized screen. The deck is drawn inside an `iframe` of 915x412 px, so that everything measured in
 * viewport units (the text of widgets) is exactly as on the phone, and the whole is scaled to fit the window.
 */
export default function PhoneFrame({ availableWidth, display, children, view, maxHeight }: Props) {
  const own = view ?? phoneView(display.orientation);
  const viewW = own.w;
  const viewH = own.h;
  const frame = useRef<HTMLIFrameElement>(null);
  const [doc, setDoc] = useState<Document | null>(null);
  const scale = fitScale(own, availableWidth, maxHeight);

  // Once the iframe exists: give it the window's styles (the deck's own sheets are among them) and a black screen.
  useEffect(() => {
    const el = frame.current;
    if (!el) return;
    const ready = () => {
      const d = el.contentDocument;
      if (!d?.body) return;
      d.head.querySelectorAll("[data-copied]").forEach((n) => n.remove());
      document.head.querySelectorAll('style, link[rel="stylesheet"]').forEach((node) => {
        const copy = node.cloneNode(true) as HTMLElement;
        copy.setAttribute("data-copied", "");
        d.head.appendChild(copy);
      });
      const base = d.createElement("style");
      base.setAttribute("data-copied", "");
      base.textContent = "html,body{margin:0;height:100%;overflow:hidden;background:var(--screen-bg,#000);color:#e5e5e5;font-family:system-ui,sans-serif;user-select:none}";
      d.head.appendChild(base);
      setDoc(d);
    };
    if (el.contentDocument?.readyState === "complete" && el.contentDocument.body) ready();
    el.addEventListener("load", ready);
    return () => el.removeEventListener("load", ready);
  }, []);

  useEffect(() => {
    if (doc) applyTheme(doc.documentElement, display);
  }, [doc, display]);

  return (
    <div className="phone-frame" style={{ width: viewW * scale, height: viewH * scale }}>
      <iframe
        ref={frame}
        title={t("frame.title")}
        srcDoc={SRC_DOC}
        style={{ width: viewW, height: viewH, transform: `scale(${scale})`, transformOrigin: "top left" }}
      />
      {doc && createPortal(children, doc.body)}
    </div>
  );
}
