import { useEffect, useMemo, useRef, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { icons as lucideIcons } from "lucide-react";
import { t } from "@decka/shared";
import { call } from "../ipc";
import Button from "../ui/Button";
import Select from "../ui/Select";
import { ICON_NAMES } from "./iconNames";
import { brandUrl, loadBrands, searchBrands, type Brand } from "./brands";
import { searchIcons, svgUrl, visibleRows, WEIGHTS, type Weight } from "./phosphor";

/** `volume-2` -> `Volume2`, the key of lucide-react's `icons`. */
function pascal(name: string): string {
  return name.split("-").map((p) => p.charAt(0).toUpperCase() + p.slice(1)).join("");
}

type LucideGlyph = React.ComponentType<{ size?: number }>;
const lucideGlyph = (name: string): LucideGlyph | undefined => (lucideIcons as Record<string, LucideGlyph>)[pascal(name)];

/** A vector picture drawn as a mask in the text colour (so it fits light and dark windows). */
function MaskIcon({ url, size }: { url: string; size: number }) {
  const mask = `url("${url}")`;
  return (
    <span
      aria-hidden="true"
      style={{
        display: "block",
        width: size,
        height: size,
        backgroundColor: "currentColor",
        WebkitMaskImage: mask,
        maskImage: mask,
        WebkitMaskSize: "contain",
        maskSize: "contain",
        WebkitMaskRepeat: "no-repeat",
        maskRepeat: "no-repeat",
        WebkitMaskPosition: "center",
        maskPosition: "center",
      }}
    />
  );
}

/** What the current icon looks like: the picture, the Lucide glyph, the emoji, or an empty box. */
export function IconPreview({ icon }: { icon: string | undefined }) {
  const [url, setUrl] = useState<string | null>(null);
  useEffect(() => {
    setUrl(null);
    if (!icon?.startsWith("img:")) return;
    let live = true;
    call("get_icon_preview", { icon }).then((u) => live && setUrl(u), () => live && setUrl(null));
    return () => {
      live = false;
    };
  }, [icon]);

  let content: React.ReactNode = null;
  if (icon?.startsWith("img:")) {
    if (!url) content = <span className="muted">?</span>;
    else if (url.startsWith("data:image/svg+xml")) content = <MaskIcon url={url} size={24} />;
    else content = <img src={url} width={32} height={32} alt="" />;
  } else if (icon?.startsWith("lucide:")) {
    const Glyph = lucideGlyph(icon.slice(7));
    content = Glyph ? <Glyph size={24} /> : <span className="muted">{icon.slice(7)}</span>;
  } else if (icon?.startsWith("emoji:")) content = <span style={{ fontSize: 22 }}>{icon.slice(6)}</span>;
  return <span className="icon-preview">{content}</span>;
}

const CELL = 48;
const VIEW_H = 288;

/** The grid takes as many columns as the box is wide and stays centred (no empty strip on the right). */
function useGridBox() {
  const [width, setWidth] = useState(CELL * 8);
  const box = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const el = box.current;
    if (!el) return;
    const measure = () => setWidth(el.clientWidth);
    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(el);
    return () => observer.disconnect();
  }, []);
  const COLS = Math.max(4, Math.floor(width / CELL));
  return { box, COLS, pad: Math.max(0, Math.floor((width - COLS * CELL) / 2)) };
}

/** Logos of programs and services (Simple Icons): search, and a grid that renders only the rows in view. */
function BrandPicker({ onPick, busy }: { onPick: (svg: string, hex: string) => void; busy: boolean }) {
  const [query, setQuery] = useState("");
  const [scrollTop, setScrollTop] = useState(0);
  const [error, setError] = useState<string | null>(null);
  const [all, setAll] = useState<Brand[]>([]);
  const { box, COLS, pad } = useGridBox();
  useEffect(() => {
    loadBrands().then(setAll, (e) => setError(String(e)));
  }, []);
  const found = useMemo(() => searchBrands(all, query), [all, query]);
  const rows = Math.ceil(found.length / COLS);
  const [first, end] = visibleRows(scrollTop, VIEW_H, CELL, rows);

  async function pick(brand: Brand) {
    setError(null);
    try {
      const response = await fetch(brandUrl(brand.slug));
      if (!response.ok) throw new Error(`cannot load ${brand.title}`);
      // The logo is a single black path; `currentColor` lets the key's text colour paint it like the other icons.
      onPick((await response.text()).replace("<svg ", '<svg fill="currentColor" '), brand.hex);
    } catch (e) {
      setError(String(e));
    }
  }

  const cells = [];
  for (let row = first; row < end; row++) {
    for (let col = 0; col < COLS; col++) {
      const brand = found[row * COLS + col];
      if (!brand) break;
      cells.push(
        <button key={brand.slug} type="button" className="phosphor-cell" title={brand.title} disabled={busy} style={{ top: row * CELL, left: pad + col * CELL, width: CELL, height: CELL }} onClick={() => void pick(brand)}>
          <MaskIcon url={brandUrl(brand.slug)} size={26} />
        </button>,
      );
    }
  }

  return (
    <div className="stack">
      <input aria-label={t("icon.search")} placeholder={t("icon.search_brands", { n: all.length })} value={query} onChange={(e) => { setQuery(e.target.value); setScrollTop(0); }} />
      <div ref={box} className="phosphor-scroll" style={{ height: VIEW_H }} onScroll={(e) => setScrollTop(e.currentTarget.scrollTop)}>
        <div style={{ position: "relative", height: rows * CELL, width: "100%" }}>{cells}</div>
        {all.length > 0 && found.length === 0 && <p className="muted">{t("icon.none_match")}</p>}
      </div>
      {error && <p className="warn-text">{error}</p>}
    </div>
  );
}

/** `lucide:volume-2` -> `Volume 2`; the raw id is for the file, not for the person. */
function iconLabel(icon: string | undefined): string {
  if (!icon) return t("icon.no_icon");
  if (icon.startsWith("lucide:")) return icon.slice(7).split("-").map((p) => p.charAt(0).toUpperCase() + p.slice(1)).join(" ");
  if (icon.startsWith("emoji:")) return icon.slice(6);
  return t("icon.tab_image");
}

/** The Phosphor library: search, a weight, and a grid that renders only the rows in view. */
function PhosphorPicker({ onPick, busy }: { onPick: (svg: string) => void; busy: boolean }) {
  const [query, setQuery] = useState("");
  const [weight, setWeight] = useState<Weight>("regular");
  const [scrollTop, setScrollTop] = useState(0);
  const [error, setError] = useState<string | null>(null);
  const { box, COLS, pad } = useGridBox();
  const found = useMemo(() => searchIcons(query), [query]);
  const rows = Math.ceil(found.length / COLS);
  const [first, end] = visibleRows(scrollTop, VIEW_H, CELL, rows);

  async function pick(name: string) {
    setError(null);
    try {
      const response = await fetch(svgUrl(name, weight));
      if (!response.ok) throw new Error(`cannot load ${name}`);
      onPick(await response.text());
    } catch (e) {
      setError(String(e));
    }
  }

  const cells = [];
  for (let row = first; row < end; row++) {
    for (let col = 0; col < COLS; col++) {
      const entry = found[row * COLS + col];
      if (!entry) break;
      cells.push(
        <button
          key={entry.name}
          type="button"
          className="phosphor-cell"
          title={entry.name}
          disabled={busy}
          style={{ top: row * CELL, left: pad + col * CELL, width: CELL, height: CELL }}
          onClick={() => void pick(entry.name)}
        >
          <MaskIcon url={svgUrl(entry.name, weight)} size={26} />
        </button>,
      );
    }
  }

  return (
    <div className="stack">
      <div className="row">
        <input aria-label={t("icon.search")} placeholder={t("icon.search_n", { n: found.length })} value={query} onChange={(e) => { setQuery(e.target.value); setScrollTop(0); }} />
        <Select
          aria-label={t("icon.weight")}
          value={weight}
          options={WEIGHTS.map((w) => ({ value: w, label: w }))}
          onChange={(val) => setWeight(val as Weight)}
        />
      </div>
      <div ref={box} className="phosphor-scroll" style={{ height: VIEW_H }} onScroll={(e) => setScrollTop(e.currentTarget.scrollTop)}>
        <div style={{ position: "relative", height: rows * CELL, width: "100%" }}>{cells}</div>
        {found.length === 0 && <p className="muted">{t("icon.none_match")}</p>}
      </div>
      {error && <p className="warn-text">{error}</p>}
    </div>
  );
}

type Tab = "brands" | "phosphor" | "quick" | "program" | "image" | "emoji";

interface Props {
  value: string | undefined;
  onChange: (icon: string | undefined) => void;
  /** Program of the button's `launch` action; enables the `Program` tab. `undefined` hides it (state looks). */
  program?: string | null;
  /** A brand logo was picked: the icon and its official colour (`#rrggbb`) in one call, so the key can take both. */
  onBrand?: (icon: string, hex: string) => void;
}

/** Icon of a button: Phosphor, the quick Lucide set, the icon of the action's program, a picture file, or an emoji. */
export default function IconField({ value, onChange, program, onBrand }: Props) {
  const [tab, setTab] = useState<Tab>("brands"); // the main set: logos of programs and services
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [emoji, setEmoji] = useState("");

  async function fromResult(promise: Promise<{ icon: string } | { error: string }>, apply: (icon: string) => void = onChange) {
    setBusy(true);
    setError(null);
    try {
      const result = await promise;
      if ("icon" in result) apply(result.icon);
      else setError(result.error);
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }

  async function fromImage() {
    const picked = await open({ multiple: false, directory: false, filters: [{ name: t("icon.images"), extensions: ["png", "jpg", "jpeg", "ico", "bmp"] }] });
    if (typeof picked === "string") await fromResult(call("icon_from_image", { path: picked }));
  }

  const tabs: [Tab, string][] = [["brands", t("icon.tab_brands")], ["quick", t("icon.tab_quick")], ["phosphor", "Phosphor"], ...(program !== undefined ? ([["program", t("icon.tab_program")]] as [Tab, string][]) : []), ["image", t("icon.tab_image")], ["emoji", t("icon.tab_emoji")]];

  return (
    <div className="icon-field">
      <div className="row">
        <IconPreview icon={value} />
        <span className="icon-value" title={value}>{iconLabel(value)}</span>
        <Button disabled={value === undefined} onClick={() => { setError(null); onChange(undefined); }}>
          {t("item.clear")}
        </Button>
      </div>
      <div className="row wrap" role="tablist">
        {tabs.map(([id, label]) => (
          <button key={id} type="button" role="tab" aria-selected={tab === id} className={tab === id ? "page-tab active" : "page-tab"} onClick={() => setTab(id)}>
            {label}
          </button>
        ))}
      </div>

      {tab === "brands" && (
        <BrandPicker
          busy={busy}
          onPick={(svg, hex) => {
            // One change with both the icon and the colour: two separate ones would overwrite each other.
            void fromResult(call("icon_from_svg", { svg }), (icon) => (onBrand ? onBrand(icon, `#${hex.toLowerCase()}`) : onChange(icon)));
          }}
        />
      )}
      {tab === "phosphor" && <PhosphorPicker busy={busy} onPick={(svg) => void fromResult(call("icon_from_svg", { svg }))} />}
      {tab === "quick" && (
        <div className="quick-grid">
          {ICON_NAMES.map((n) => {
            const Glyph = lucideGlyph(n);
            return (
              <button key={n} type="button" className="phosphor-cell quick-cell" title={n} onClick={() => onChange(`lucide:${n}`)}>
                {Glyph ? <Glyph size={22} /> : n}
              </button>
            );
          })}
        </div>
      )}
      {tab === "program" && (
        <div className="row">
          <Button disabled={busy || !program} onClick={() => void fromResult(call("icon_from_program", { path: program as string }))}>
            {t("icon.use_program")}
          </Button>
          {!program && <span className="muted">{t("icon.need_launch")}</span>}
        </div>
      )}
      {tab === "image" && (
        <div className="row">
          <Button disabled={busy} onClick={() => void fromImage()}>
            {t("icon.from_image")}
          </Button>
          <span className="muted">{t("icon.image_types")}</span>
        </div>
      )}
      {tab === "emoji" && (
        <div className="row">
          <input aria-label={t("icon.tab_emoji")} placeholder="🚀" value={emoji} maxLength={8} onChange={(e) => setEmoji(e.target.value)} />
          <Button disabled={emoji.trim() === ""} onClick={() => onChange(`emoji:${emoji.trim()}`)}>
            {t("icon.use_emoji")}
          </Button>
        </div>
      )}
      {error && <p className="warn-text">{error}</p>}
    </div>
  );
}
