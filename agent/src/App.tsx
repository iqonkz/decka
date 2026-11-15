import { useCallback, useEffect, useState } from "react";
import { call, onNavigate } from "./ipc";
import AppearanceTab from "./tabs/AppearanceTab";
import FirstRun, { wizardDone } from "./tabs/FirstRun";
import LayoutTab from "./tabs/LayoutTab";
import MoreTab, { isMoreSection, type MoreSection } from "./tabs/MoreTab";
import PhoneTab from "./tabs/PhoneTab";
import Tabs, { type TabDef } from "./ui/Tabs";

type TabId = "deck" | "look" | "phone" | "more";

const TABS: TabDef<TabId>[] = [
  { id: "deck", label: "Deck", icon: "squares-four" },
  { id: "look", label: "Look", icon: "palette" },
  { id: "phone", label: "Phone", icon: "device-mobile" },
  { id: "more", label: "More", icon: "dots-three-outline" },
];

const isTab = (s: string): s is TabId => TABS.some((t) => t.id === s);

/**
 * Where a name goes: the new four tabs, and the old names of the tray, the editor and the first-run guide
 * (`status`, `pairing` -> Phone; `appearance` -> Look; `layout` -> Deck; `actions`, `integrations`, `settings` -> More).
 */
export function resolveTarget(target: string): { tab: TabId; section?: MoreSection } | null {
  const name = target.split(":")[0] ?? "";
  if (isTab(name)) return { tab: name };
  switch (name) {
    case "status":
    case "pairing":
      return { tab: "phone" };
    case "appearance":
      return { tab: "look" };
    case "layout":
      return { tab: "deck" };
    default:
      return isMoreSection(name) ? { tab: "more", section: name } : null;
  }
}

function TabBody({ tab, section, onSection }: { tab: TabId; section: MoreSection; onSection: (s: MoreSection) => void }) {
  switch (tab) {
    case "deck":
      return <LayoutTab />;
    case "look":
      return <AppearanceTab />;
    case "phone":
      return <PhoneTab />;
    case "more":
      return <MoreTab section={section} onSection={onSection} />;
  }
}
export default function App() {
  const [tab, setTab] = useState<TabId>("deck");
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
  const refreshAccent = useCallback(() => {
    call("get_layout").then(
      (l) => document.documentElement.style.setProperty("--accent", l.display.accent ?? "#7c5cff"),
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

  return (
    <div className="shell">
      <Tabs tabs={TABS} active={tab} onSelect={setTab} />
      <main className="content">
        <TabBody tab={tab} section={section} onSection={setSection} />
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
