import { t } from "@decka/shared";
import ActionsTab from "./ActionsTab";
import IntegrationsTab from "./IntegrationsTab";
import SettingsTab from "./SettingsTab";
import Button from "../ui/Button";

export type MoreSection = "actions" | "integrations" | "settings" | "about";

const SECTION_IDS: MoreSection[] = ["actions", "integrations", "settings", "about"];

function sectionLabel(id: MoreSection): string {
  return t(`more.${id}`);
}

export const isMoreSection = (s: string): s is MoreSection => (SECTION_IDS as string[]).includes(s);

function About() {
  return (
    <div className="stack">
      <h2>{t("more.about")}</h2>
      <section className="card">
        <p>{t("about.text")}</p>
        <p className="muted">{t("about.licence")}</p>
        <Button onClick={() => window.dispatchEvent(new CustomEvent("decka:run-setup", { detail: { step: 0 } }))}>{t("about.run_setup")}</Button>
      </section>
    </div>
  );
}

/** The rarely needed pages: actions, integrations, settings, about (DESIGN.md 4.6a). */
export default function MoreTab({ section, onSection }: { section: MoreSection; onSection: (s: MoreSection) => void }) {
  return (
    <div className="more-layout">
      <nav className="more-nav" aria-label={t("tab.more")}>
        {SECTION_IDS.map((id) => (
          <button key={id} type="button" className={id === section ? "active" : ""} title={sectionLabel(id)} onClick={() => onSection(id)}>
            {sectionLabel(id)}
          </button>
        ))}
      </nav>
      <div className="more-body">
        {section === "actions" && <ActionsTab />}
        {section === "integrations" && <IntegrationsTab />}
        {section === "settings" && <SettingsTab />}
        {section === "about" && <About />}
      </div>
    </div>
  );
}
