import { useEffect, useRef, useState } from "react";
import { t, type TranslationKey } from "@decka/shared";
import logoUrl from "../../../docs/assets/decka-logo.svg";
import { call } from "../ipc";
import PhIcon from "./PhIcon";
import { revealAnchor, searchRegistry, targetForPage, type SearchEntry, type SearchTargetPage } from "./searchRegistry";

export interface SidebarGroupDef {
  id: string;
  titleKey: TranslationKey;
  items: {
    id: SearchTargetPage;
    titleKey: TranslationKey;
    icon: string;
    dot?: boolean;
  }[];
}

/** Localized menu name of a page, for the badge of a search result. */
function pageTitle(page: SearchTargetPage): string {
  const item = sidebarGroups()
    .flatMap((g) => g.items)
    .find((i) => i.id === page);
  return item ? t(item.titleKey) : page;
}

export function sidebarGroups(updateAvailable?: boolean): SidebarGroupDef[] {
  return [
    {
      id: "deck",
      titleKey: "sidebar.group.deck",
      items: [
        { id: "deck", titleKey: "sidebar.item.keys", icon: "squares-four" },
        { id: "look", titleKey: "sidebar.item.look", icon: "palette" },
        { id: "phone", titleKey: "sidebar.item.phone", icon: "device-mobile" },
      ],
    },
    {
      id: "assistant",
      titleKey: "sidebar.group.assistant",
      items: [
        { id: "voice", titleKey: "sidebar.item.voice", icon: "microphone" },
        { id: "reminders", titleKey: "sidebar.item.reminders", icon: "bell" },
      ],
    },
    {
      id: "connections",
      titleKey: "sidebar.group.connections",
      items: [
        { id: "integrations", titleKey: "sidebar.item.integrations", icon: "plugs-connected" },
        { id: "actions", titleKey: "sidebar.item.actions", icon: "lightning" },
      ],
    },
    {
      id: "app",
      titleKey: "sidebar.group.app",
      items: [
        { id: "settings", titleKey: "sidebar.item.settings", icon: "gear-six" },
        { id: "about", titleKey: "sidebar.item.about", icon: "info", dot: updateAvailable },
      ],
    },
  ];
}

export interface SidebarProps {
  activePage: SearchTargetPage;
  onNavigate: (target: { tab: "deck" | "look" | "phone" | "more"; section?: "actions" | "integrations" | "settings" | "voice" | "reminders" | "about" }) => void;
  updateAvailable?: boolean;
}

export default function Sidebar({ activePage, onNavigate, updateAvailable }: SidebarProps) {
  const [collapsed, setCollapsed] = useState<boolean>(() => {
    try {
      return localStorage.getItem("decka.sidebar.v1") === "collapsed";
    } catch {
      return false;
    }
  });

  const [query, setQuery] = useState("");
  const [version, setVersion] = useState<string | null>(null);

  useEffect(() => {
    call("get_status").then((s) => setVersion(s.version), () => {});
  }, []);
  const [results, setResults] = useState<SearchEntry[]>([]);
  const [selectedIndex, setSelectedIndex] = useState(0);
  const [showDropdown, setShowDropdown] = useState(false);
  const searchWrapRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  // Auto-collapse when window < 1100 px (DESIGN.md 4.14)
  useEffect(() => {
    const mql = window.matchMedia("(max-width: 1099px)");
    const onChange = (e: MediaQueryListEvent | MediaQueryList) => {
      if (e.matches) {
        setCollapsed(true);
      }
    };
    mql.addEventListener("change", onChange);
    if (mql.matches) setCollapsed(true);
    return () => mql.removeEventListener("change", onChange);
  }, []);

  const toggleCollapsed = () => {
    const next = !collapsed;
    setCollapsed(next);
    try {
      localStorage.setItem("decka.sidebar.v1", next ? "collapsed" : "expanded");
    } catch {}
  };

  const onSearchChange = (val: string) => {
    setQuery(val);
    if (val.trim()) {
      const res = searchRegistry(val);
      setResults(res);
      setSelectedIndex(0);
      setShowDropdown(true);
    } else {
      setResults([]);
      setShowDropdown(false);
    }
  };

  const handleSelectResult = (entry: SearchEntry) => {
    setShowDropdown(false);
    setQuery("");
    onNavigate(targetForPage(entry.page));

    if (entry.anchor) {
      // The page is mounted (and may load its data) after the click: look for the element for up to 2 s.
      let tries = 0;
      const look = () => {
        if (!revealAnchor(entry) && ++tries < 20) setTimeout(look, 100);
      };
      setTimeout(look, 100);
    }
  };

  const onKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key === "Escape") {
      setShowDropdown(false);
      setQuery("");
      return;
    }

    if (!showDropdown || results.length === 0) return;

    if (e.key === "ArrowDown") {
      e.preventDefault();
      setSelectedIndex((prev) => Math.min(prev + 1, results.length - 1));
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      setSelectedIndex((prev) => Math.max(prev - 1, 0));
    } else if (e.key === "Enter") {
      e.preventDefault();
      const selected = results[selectedIndex] ?? results[0];
      if (selected) handleSelectResult(selected);
    }
  };

  // Close search dropdown on click outside
  useEffect(() => {
    const onPointerDown = (e: PointerEvent) => {
      if (searchWrapRef.current && !searchWrapRef.current.contains(e.target as Node)) {
        setShowDropdown(false);
      }
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") setShowDropdown(false);
    };
    window.addEventListener("pointerdown", onPointerDown);
    window.addEventListener("keydown", onKey);
    return () => {
      window.removeEventListener("pointerdown", onPointerDown);
      window.removeEventListener("keydown", onKey);
    };
  }, []);

  const groups = sidebarGroups(updateAvailable);

  return (
    <aside className={`sidebar ${collapsed ? "collapsed" : ""}`}>
      {/* Brand header */}
      <div className="sidebar-header">
        <img src={logoUrl} width={24} height={24} alt="" />
        <div className="sidebar-brand-info">
          <span className="sidebar-brand-name">Decka</span>
          {version && <span className="sidebar-version">{version}</span>}
        </div>
      </div>

      {/* Search box */}
      <div className="sidebar-search-wrap" ref={searchWrapRef}>
        {collapsed ? (
          <button
            type="button"
            className="sidebar-search-btn-collapsed"
            aria-label={t("sidebar.search")}
            title={t("sidebar.search")}
            onClick={() => {
              setCollapsed(false);
              setTimeout(() => inputRef.current?.focus(), 150);
            }}
          >
            <PhIcon name="magnifying-glass" size={16} />
          </button>
        ) : (
          <div className="sidebar-search-box">
            <span className="sidebar-search-icon" aria-hidden="true">
              <PhIcon name="magnifying-glass" size={16} />
            </span>
            <input
              ref={inputRef}
              type="text"
              className="sidebar-search-input"
              placeholder={t("sidebar.search")}
              value={query}
              onChange={(e) => onSearchChange(e.target.value)}
              onKeyDown={onKeyDown}
              onFocus={() => {
                if (query.trim()) setShowDropdown(true);
              }}
            />
            {query && (
              <button
                type="button"
                className="sidebar-search-clear"
                aria-label={t("item.clear")}
                onClick={() => onSearchChange("")}
              >
                <PhIcon name="x" size={14} />
              </button>
            )}
          </div>
        )}

        {/* Dropdown results */}
        {!collapsed && showDropdown && (
          <ul className="sidebar-search-results" role="listbox">
            {results.length > 0 ? (
              results.map((r, i) => (
                <li
                  key={r.id}
                  role="option"
                  aria-selected={i === selectedIndex}
                  className={`sidebar-search-item ${i === selectedIndex ? "selected" : ""}`}
                  onClick={() => handleSelectResult(r)}
                  onMouseEnter={() => setSelectedIndex(i)}
                >
                  <span className="sidebar-search-item-title">{t(r.titleKey)}</span>
                  <span className="sidebar-search-item-badge">{pageTitle(r.page)}</span>
                </li>
              ))
            ) : (
              <li className="sidebar-search-empty">{t("sidebar.nothing_found")}</li>
            )}
          </ul>
        )}
      </div>

      {/* Groups & Items */}
      <nav className="sidebar-groups" aria-label={t("more.settings")}>
        {groups.map((g) => (
          <div key={g.id} className="sidebar-group">
            <div className="sidebar-group-title">{t(g.titleKey)}</div>
            <div className="sidebar-items">
              {g.items.map((item) => {
                const isActive = item.id === activePage;
                const label = t(item.titleKey);
                return (
                  <button
                    key={item.id}
                    type="button"
                    className={`sidebar-item ${isActive ? "active" : ""}`}
                    title={collapsed ? label : undefined}
                    aria-current={isActive ? "page" : undefined}
                    onClick={() => onNavigate(targetForPage(item.id))}
                  >
                    <PhIcon name={item.icon} size={18} />
                    <span className="sidebar-item-label">{label}</span>
                    {item.dot && <span className="sidebar-item-dot" aria-hidden="true" />}
                  </button>
                );
              })}
            </div>
          </div>
        ))}
      </nav>

      {/* Collapse button at bottom */}
      <div className="sidebar-footer">
        <button
          type="button"
          className="sidebar-collapse-btn"
          title={collapsed ? t("sidebar.expand") : t("sidebar.collapse")}
          aria-label={collapsed ? t("sidebar.expand") : t("sidebar.collapse")}
          onClick={toggleCollapsed}
        >
          <PhIcon name="sidebar-simple" size={20} />
          <span>{collapsed ? t("sidebar.expand") : t("sidebar.collapse")}</span>
        </button>
      </div>
    </aside>
  );
}
