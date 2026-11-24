import { useCallback, useEffect, useState } from "react";
import { t, type LanguageSetting } from "@decka/shared";
import { useApplyLanguage } from "@decka/ui";
import { call, onNavigate } from "./ipc";
import { LANGUAGE_EVENT } from "./language";
import UpdateBanner from "./tabs/UpdateBanner";
import { useUpdates } from "./updates";
import AppearanceTab from "./tabs/AppearanceTab";
import FirstRun, { wizardDone } from "./tabs/FirstRun";
import LayoutTab from "./tabs/LayoutTab";
import MoreTab, { isMoreSection, type MoreSection } from "./tabs/MoreTab";
import PhoneTab from "./tabs/PhoneTab";
import Sidebar from "./ui/Sidebar";
import type { SearchTargetPage } from "./ui/searchRegistry";

type TabId = "deck" | "look" | "phone" | "more";

const TAB_IDS: TabId[] = ["deck", "look", "phone", "more"];

const isTab = (s: string): s is TabId => (TAB_IDS as string[]).includes(s);

/**
 * Where a name goes: the tabs and sections of the sidebar menu, and the old names of the tray,
 * the editor and the first-run guide (`status`, `pairing` -> Phone; `appearance` -> Look; `layout`/`keys` -> Deck; `actions`, `integrations`, `settings`, `voice`, `reminders`, `about` -> More).
 */
export function resolveTarget(target: string): { tab: TabId; section?: MoreSection } | null {
  const name = target.split(":")[0] ?? "";
  if (isTab(name)) return { tab: name };
  switch (name) {
    case "keys":
    case "layout":
      return { tab: "deck" };
    case "status":
    case "pairing":
      return { tab: "phone" };
    case "appearance":
      return { tab: "look" };
    default:
      return isMoreSection(name) ? { tab: "more", section: name } : null;
  }
}

function getActivePage(tab: TabId, section: MoreSection): SearchTargetPage {
  if (tab === "deck") return "deck";
  if (tab === "look") return "look";
  if (tab === "phone") return "phone";
  return section;
}

function TabBody({ tab, section, updates }: { tab: TabId; section: MoreSection; updates: ReturnType<typeof useUpdates> }) {
  switch (tab) {
    case "deck":
      return <LayoutTab />;
    case "look":
      return <AppearanceTab />;
    case "phone":
      return <PhoneTab />;
    case "more":
      return <MoreTab section={section} updates={updates} />;
  }
}
export default function App() {
  const [tab, setTab] = useState<TabId>("deck");
  const updates = useUpdates();
  const [section, setSection] = useState<MoreSection>("actions");
  const open = useCallback((target: string) => {
    const r = resolveTarget(target);
    if (!r) return;
    setTab(r.tab);
    if (r.section) setSection(r.section);
  }, []);
  const [firstRun, setFirstRun] = useState(false);
  const [wizardStep, setWizardStep] = useState(0);

  // Other tabs ask for the guide (`Run setup again`, the empty page) or for another tab through window events.
  useEffect(() => {
    const run = (e: Event) => {
      setWizardStep((e as CustomEvent<{ step?: number }>).detail?.step ?? 0);
      setFirstRun(true);
    };
    const go = (e: Event) => {
      open((e as CustomEvent<{ tab?: string }>).detail?.tab ?? "");
    };
    window.addEventListener("decka:run-setup", run);
    window.addEventListener("decka:go", go);
    return () => {
      window.removeEventListener("decka:run-setup", run);
      window.removeEventListener("decka:go", go);
    };
  }, []);

  // The accent of the deck is also the accent of this window.
  // The language comes from the same layout (`display.language`; `auto` = the system language of this window).
  const [languageSetting, setLanguageSetting] = useState<LanguageSetting | undefined>(undefined);
  useApplyLanguage(languageSetting, navigator.language);
  useEffect(() => {
    const changed = (e: Event) => setLanguageSetting((e as CustomEvent<{ language: LanguageSetting }>).detail.language);
    window.addEventListener(LANGUAGE_EVENT, changed);
    return () => window.removeEventListener(LANGUAGE_EVENT, changed);
  }, []);
  const refreshAccent = useCallback(() => {
    call("get_layout").then(
      (l) => {
        document.documentElement.style.setProperty("--accent", l.display.accent ?? "#7c5cff");
        setLanguageSetting(l.display.language);
      },
      () => {},
    );
  }, []);
  useEffect(refreshAccent, [tab, refreshAccent]);

  // The first-run guide: nothing paired, the layout untouched, and not dismissed before.
  useEffect(() => {
    if (wizardDone()) return;
    call("is_first_run").then((first) => setFirstRun(first), () => {});
  }, []);

  // The tray asks for a tab through `decka://navigate`.
  useEffect(() => {
    let off: (() => void) | null = null;
    let disposed = false;
    onNavigate((target) => open(target)).then((unlisten) => {
      if (disposed) unlisten();
      else off = unlisten;
    });
    return () => {
      disposed = true;
      off?.();
    };
  }, []);

  const handleSidebarNavigate = useCallback((target: { tab: TabId; section?: MoreSection }) => {
    setTab(target.tab);
    if (target.section) setSection(target.section);
  }, []);

  return (
    <div className="shell">
      <Sidebar
        activePage={getActivePage(tab, section)}
        onNavigate={handleSidebarNavigate}
        updateAvailable={updates.status?.available != null}
      />
      <main className="content">
        <UpdateBanner status={updates.status} />
        <TabBody tab={tab} section={section} updates={updates} />
      </main>
      {firstRun && (
        <FirstRun
          key={wizardStep}
          startStep={wizardStep}
          onClose={() => {
            setFirstRun(false);
            refreshAccent();
          }}
        />
      )}
    </div>
  );
}
