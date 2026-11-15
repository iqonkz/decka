import { useEffect, useRef, useState, type ReactNode } from "react";
import { createPortal } from "react-dom";
import type { DisplaySettings } from "@decka/shared";
import { applyTheme } from "@decka/ui";

/** The viewport of the preview: the phone in landscape (CSS pixels). */
export const PHONE_W = 915;
export const PHONE_H = 412;

interface Props {
  /** Width in px the frame may take; it is scaled down to fit and never enlarged. */
  availableWidth: number;
  display: DisplaySettings;
  children: ReactNode;
  /** Another screen size than the phone (the key preview of the properties panel). */
  view?: { w: number; h: number };
}

const SRC_DOC = `<!doctype html><html><head><meta charset="utf-8"></head><body></body></html>`;

/**
 * A phone-sized screen. The deck is drawn inside an `iframe` of 915x412 px, so that everything measured in
 * viewport units (the text of widgets) is exactly as on the phone, and the whole is scaled to fit the window.
 */
export default function PhoneFrame({ availableWidth, display, children, view }: Props) {
  const viewW = view?.w ?? PHONE_W;
  const viewH = view?.h ?? PHONE_H;
  const frame = useRef<HTMLIFrameElement>(null);
  const [doc, setDoc] = useState<Document | null>(null);
  const scale = Math.max(0.2, Math.min(1, availableWidth / viewW));

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
        title="Layout preview"
        srcDoc={SRC_DOC}
        style={{ width: viewW, height: viewH, transform: `scale(${scale})`, transformOrigin: "top left" }}
      />
      {doc && createPortal(children, doc.body)}
    </div>
  );
}
