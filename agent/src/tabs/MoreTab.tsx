import ActionsTab from "./ActionsTab";
import IntegrationsTab from "./IntegrationsTab";
import SettingsTab from "./SettingsTab";
import Button from "../ui/Button";

export type MoreSection = "actions" | "integrations" | "settings" | "about";

const SECTIONS: { id: MoreSection; label: string }[] = [
  { id: "actions", label: "Actions" },
  { id: "integrations", label: "Integrations" },
  { id: "settings", label: "Settings" },
  { id: "about", label: "About" },
];

export const isMoreSection = (s: string): s is MoreSection => SECTIONS.some((x) => x.id === s);

function About() {
  return (
    <div className="stack">
      <h2>About</h2>
      <section className="card">
        <p>Decka turns an old phone into a panel for this PC: buttons, widgets and live information.</p>
        <p className="muted">MIT licence. The setup guide pairs a phone, picks a look and adds your apps.</p>
        <Button onClick={() => window.dispatchEvent(new CustomEvent("decka:run-setup", { detail: { step: 0 } }))}>Run setup again</Button>
      </section>
    </div>
  );
}

/** The rarely needed pages: actions, integrations, settings, about (DESIGN.md 4.6a). */
export default function MoreTab({ section, onSection }: { section: MoreSection; onSection: (s: MoreSection) => void }) {
  return (
    <div className="more-layout">
      <nav className="more-nav" aria-label="More">
        {SECTIONS.map((s) => (
          <button key={s.id} type="button" className={s.id === section ? "active" : ""} onClick={() => onSection(s.id)}>
            {s.label}
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