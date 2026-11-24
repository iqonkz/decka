import { t, type MailItem } from "@decka/shared";
import { useDeckEnv } from "../deckEnv";
import Icon from "../Icon";
import WidgetEmpty from "./WidgetEmpty";

/**
 * Unread mail (DATA_MODEL.md 10.7, DESIGN.md 4.16).
 */
export default function Mail({ item }: { item: MailItem }) {
  const { mail, online, onPress, onPressError } = useDeckEnv();
  const accounts = mail?.accounts ?? [];
  const open = (actionId: string | undefined) => {
    if (actionId !== undefined && online) onPress(actionId).catch(onPressError);
  };
  if (accounts.length === 0) {
    return <WidgetEmpty icon="lucide:mail" text={t("mail.none")} />;
  }
  if (item.w === 1 && item.h === 1) {
    const total = accounts.reduce((sum, a) => sum + (a.unread ?? 0), 0);
    return (
      <div className={total === 0 ? "mail mail-one dim" : "mail mail-one"} onClick={() => open(accounts[0]?.openActionId)}>
        <Icon icon="lucide:mail" size={22} />
        {total > 0 && <strong className="widget-figure">{total}</strong>}
      </div>
    );
  }
  return (
    <div className="mail mail-list">
      {accounts.slice(0, item.h === 1 ? 2 : 3).map((a) => (
        <div className="mail-row" key={a.id} onClick={() => open(a.openActionId)}>
          <Icon icon="lucide:mail" size={16} />
          <span className="mail-name">{a.name}</span>
          <span className={a.error ? "mail-count warn" : "mail-count"}>{a.error ? t(`mail.error_${a.error}`) : a.unread === null ? "0" : a.unread}</span>
        </div>
      ))}
    </div>
  );
}
