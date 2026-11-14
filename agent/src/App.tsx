import { useEffect, useState } from "react";
import { onNavigate } from "./ipc";
import ActionsTab from "./tabs/ActionsTab";
import IntegrationsTab from "./tabs/IntegrationsTab";
import LayoutTab from "./tabs/LayoutTab";
import PairingTab from "./tabs/PairingTab";
import SettingsTab from "./tabs/SettingsTab";
import StatusTab from "./tabs/StatusTab";
import Tabs, { type TabDef } from "./ui/Tabs";

type TabId = "status" | "pairing" | "layout" | "actions" | "integrations" | "settings";

const TABS: TabDef<TabId>[] = [
  { id: "status", label: "Status" },
  { id: "pairing", label: "Pairing" },
  { id: "layout", label: "Layout" },
  { id: "actions", label: "Actions" },
  { id: "integrations", label: "Integrations" },
  { id: "settings", label: "Settings" },
];

const isTab = (s: string): s is TabId => TABS.some((t) => t.id === s);

function TabBody({ tab }: { tab: TabId }) {
  switch (tab) {
    case "status":
      return <StatusTab />;
    case "pairing":
      return <PairingTab />;
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
    </div>
  );
}
