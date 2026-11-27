// T31.2: why a mailbox does not work and what to do about it.
import { t, type MailReason, type MailStepId, type MailTestResult, type TranslationKey } from "@decka/shared";
import Banner from "../ui/Banner";

export const MAIL_REASONS: Record<MailReason, { title: TranslationKey; todo: TranslationKey }> = {
  auth: { title: "mailset.reason_auth", todo: "mailset.todo_auth" },
  tls: { title: "mailset.reason_tls", todo: "mailset.todo_tls" },
  host: { title: "mailset.reason_host", todo: "mailset.todo_host" },
  timeout: { title: "mailset.reason_timeout", todo: "mailset.todo_timeout" },
  app_password_required: { title: "mailset.reason_app_password_required", todo: "mailset.todo_app_password_required" },
  oauth_unsupported: { title: "mailset.reason_oauth_unsupported", todo: "mailset.todo_oauth_unsupported" },
};

const STEP_NAMES: Record<MailStepId, TranslationKey> = {
  dns: "mailset.step_dns",
  tcp: "mailset.step_tcp",
  tls: "mailset.step_tls",
  login: "mailset.step_login",
  folder: "mailset.step_folder",
};

const MARKS = {
  ok: { mark: "✓", className: "diag-ok", word: "mailset.passed" },
  error: { mark: "✗", className: "diag-fail", word: "mailset.failed" },
  skipped: { mark: "–", className: "diag-unknown", word: "mailset.skipped" },
} as const;

export function MailProblem({ reason, noPassword, detail }: { reason: MailReason; noPassword: boolean; detail?: string }) {
  const keys = noPassword ? ({ title: "mailset.reason_no_password", todo: "mailset.todo_no_password" } as const) : MAIL_REASONS[reason];
  return (
    <Banner kind="error">
      <strong className="block">{t(keys.title)}</strong>
      <span className="block">{t(keys.todo)}</span>
      {detail && <small className="muted block">{t("mailset.server_says", { text: detail })}</small>}
    </Banner>
  );
}

export function MailTestReport({ result }: { result: MailTestResult }) {
  const failed = result.steps.find((s) => s.state === "error");
  return (
    <div className="mail-test">
      <ul className="diag-list">
        {result.steps.map((s) => (
          <li key={s.id} className={MARKS[s.state].className}>
            <span className="diag-mark" aria-label={t(MARKS[s.state].word)}>{MARKS[s.state].mark}</span> {t(STEP_NAMES[s.id])}
            <span className="muted"> · {s.state === "skipped" ? t("mailset.skipped") : t("mailset.ms", { ms: s.durationMs })}</span>
          </li>
        ))}
      </ul>
      {failed?.reason ? (
        <MailProblem reason={failed.reason} noPassword={result.noPassword} detail={failed.detail} />
      ) : (
        result.unread !== null && <Banner kind="success">{t("mailset.test_ok", { n: result.unread })}</Banner>
      )}
    </div>
  );
}
