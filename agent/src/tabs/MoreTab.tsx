import { useEffect, useState } from "react";
import { locale, t } from "@decka/shared";
import logoUrl from "../../../docs/assets/decka-logo.svg";
import { call } from "../ipc";
import { releasePageOf, type useUpdates } from "../updates";
import ActionsTab from "./ActionsTab";
import UpdateBanner from "./UpdateBanner";
import IntegrationsTab from "./IntegrationsTab";
import SettingsTab from "./SettingsTab";
import VoiceSection from "./VoiceSection";
import RemindersTab from "./RemindersTab";
import Button from "../ui/Button";

type Updates = ReturnType<typeof useUpdates>;

export type MoreSection = "actions" | "integrations" | "settings" | "voice" | "reminders" | "about";

const SECTION_IDS: MoreSection[] = ["actions", "integrations", "settings", "voice", "reminders", "about"];

function sectionLabel(id: MoreSection): string {
  return t(`more.${id}`);
}

export const isMoreSection = (s: string): s is MoreSection => (SECTION_IDS as string[]).includes(s);

/** The version, what is new, and the daily check for a newer release (ARCHITECTURE.md 8a). */
function About({ updates }: { updates: Updates }) {
  const [version, setVersion] = useState<string | null>(null);
  const [enabled, setEnabled] = useState(true);
  const { status, check, checking } = updates;

  // Opening `About` asks GitHub at once (ARCHITECTURE.md 8a), unless the check is switched off.
  useEffect(() => {
    call("get_status").then((s) => setVersion(s.version), () => {});
    call("get_config").then(
      (c) => {
        setEnabled(c.updateCheck !== false);
        if (c.updateCheck !== false) void check();
      },
      () => {},
    );
  }, [check]);

  async function toggle(on: boolean) {
    setEnabled(on);
    try {
      const config = await call("get_config");
      await call("save_config", { config: { ...config, updateCheck: on } });
    } catch {
      setEnabled(!on);
    }
  }

  const checked = status?.checkedAt != null ? t("about.last_check", { time: new Date(status.checkedAt).toLocaleString(locale()) }) : t("about.never_checked");
  return (
    <div className="stack">
      <h2>{t("more.about")}</h2>
      <section className="card">
        <img className="logo-about" src={logoUrl} width={64} height={64} alt="Decka" />
        <p>
          <strong>{version ? t("about.version", { version }) : "Decka"}</strong>{" "}
          {version && (
            <button type="button" className="link" onClick={() => void call("open_release", { url: releasePageOf(version) })}>
              {t("about.changelog")}
            </button>
          )}
        </p>
        <p>{t("about.text")}</p>
        <p className="muted">{t("about.licence")}</p>
        <Button onClick={() => window.dispatchEvent(new CustomEvent("decka:run-setup", { detail: { step: 0 } }))}>{t("about.run_setup")}</Button>
      </section>
      <section className="card">
        <h3>{t("about.updates")}</h3>
        <label className="check">
          <input type="checkbox" checked={enabled} onChange={(e) => void toggle(e.target.checked)} /> {t("about.updates_toggle")}
        </label>
        <p className="muted">{t("about.updates_hint")}</p>
        <div className="row">
          <Button disabled={checking} onClick={() => void check()}>
            {checking ? t("about.checking") : t("about.check_now")}
          </Button>
          <span className="muted">{checked}</span>
        </div>
        {status?.failed && <p className="warn-text">{t("about.check_failed")}</p>}
        {status && !status.failed && status.checkedAt != null && !status.available && <p className="ok-text">{t("about.up_to_date")}</p>}
        <UpdateBanner status={status} />
      </section>
    </div>
  );
}

/** The sections of the more group, now navigated directly from the sidebar (DESIGN.md 4.14). */
export default function MoreTab({ section, updates }: { section: MoreSection; onSection?: (s: MoreSection) => void; updates: Updates }) {
  return (
    <div className="more-body">
      {section === "actions" && <ActionsTab />}
      {section === "integrations" && <IntegrationsTab />}
      {section === "settings" && <SettingsTab />}
      {section === "voice" && <VoiceSection />}
      {section === "reminders" && <RemindersTab />}
      {section === "about" && <About updates={updates} />}
    </div>
  );
}
