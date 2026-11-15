import { useCallback, useEffect, useState } from "react";
import { call, onNavigate } from "./ipc";
import ActionsTab from "./tabs/ActionsTab";
import AppearanceTab from "./tabs/AppearanceTab";
import FirstRun, { wizardDone } from "./tabs/FirstRun";
import IntegrationsTab from "./tabs/IntegrationsTab";
import LayoutTab from "./tabs/LayoutTab";
import PairingTab from "./tabs/PairingTab";
import SettingsTab from "./tabs/SettingsTab";
import StatusTab from "./tabs/StatusTab";
import Tabs, { type TabDef } from "./ui/Tabs";

type TabId = "status" | "pairing" | "appearance" | "layout" | "actions" | "integrations" | "settings";

const TABS: TabDef<TabId>[] = [
  { id: "status", label: "Status", icon: "pulse" },
  { id: "pairing", label: "Pairing", icon: "qr-code" },
  { id: "appearance", label: "Appearance", icon: "palette" },
  { id: "layout", label: "Layout", icon: "squares-four" },
  { id: "actions", label: "Actions", icon: "lightning" },
  { id: "integrations", label: "Integrations", icon: "plugs-connected" },
  { id: "settings", label: "Settings", icon: "gear" },
];

const isTab = (s: string): s is TabId => TABS.some((t) => t.id === s);

function TabBody({ tab }: { tab: TabId }) {
  switch (tab) {
    case "status":
      return <StatusTab />;
    case "pairing":
      return <PairingTab />;
    case "appearance":
      return <AppearanceTab />;
    case "actions":
      return <ActionsTab />;
    case "layout":
      return <LayoutTab />;
    case "integrations":
      return <IntegrationsTab />;
    case "settings":
      return <SettingsTab />;
  }
}

export default function App() {
  const [tab, setTab] = useState<TabId>("status");
  const [firstRun, setFirstRun] = useState(false);

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
    onNavigate((target) => {
      // "actions:new" = the Actions tab (it opens on the form for a new action).
      const tabName = target.split(":")[0] ?? "";
      if (isTab(tabName)) setTab(tabName);
    }).then((unlisten) => {
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
        <TabBody tab={tab} />
      </main>
      {firstRun && (
        <FirstRun
          onClose={() => {
            setFirstRun(false);
            refreshAccent();
          }}
        />
      )}
    </div>
  );
}
