import { createContext, useContext } from "react";
import type { MediaInfo, Notification, StatusItem, SysStats, VarValue } from "@decka/shared";

/** What the deck shows: the live sections of the agent's state. No connection, store or socket in here. */
export interface DeckData {
  /** The phone is connected: keys that run an action are usable. */
  online: boolean;
  vars: Record<string, VarValue>;
  sys: SysStats | null;
  statuses: StatusItem[];
  notifications: Notification[];
  media: MediaInfo | null;
  /** `img:` pictures by hash (the `icons` section). */
  icons: Record<string, string>;
}

export const EMPTY_DATA: DeckData = { online: false, vars: {}, sys: null, statuses: [], notifications: [], media: null, icons: {} };

/** What the deck asks its host to do. */
export interface DeckCallbacks {
  /** Press a key: resolves on `ok`, rejects with `{ code, message? }`. */
  onPress: (actionId: string) => Promise<void>;
  /** A key that failed (the host shows it, for example as a toast). */
  onPressError: (e: { code: string; message?: string }) => void;
  /** A `navigate` key: open the page with this id. */
  onNavigate: (pageId: string) => void;
  /** A notification row was tapped. */
  onDismiss: (notifId: string) => void;
}

export type DeckEnv = DeckData & DeckCallbacks;

const NOOP_ASYNC = () => Promise.resolve();
const NOOP = () => {};

export const DeckEnvContext = createContext<DeckEnv>({ ...EMPTY_DATA, onPress: NOOP_ASYNC, onPressError: NOOP, onNavigate: NOOP, onDismiss: NOOP });

export function useDeckEnv(): DeckEnv {
  return useContext(DeckEnvContext);
}
