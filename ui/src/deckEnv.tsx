import { createContext, useContext } from "react";
import type { AiInfo, BatteryInfo, ListsSection, MailInfo, MediaInfo, Notification, Page, RemindersSection, ObsInfo, StatusItem, SysStats, TwitchInfo, VarValue, WeatherInfo, YoutubeInfo } from "@decka/shared";
import type { LyricsInfo } from "./widgets/m24Types";

/** What the deck shows: the live sections of the agent's state. No connection, store or socket in here. */
export interface DeckData {
  /** The phone is connected: keys that run an action are usable. */
  online: boolean;
  /** The settings window's preview: keys never dim for missing data (DESIGN.md 2.4). */
  preview?: boolean;
  vars: Record<string, VarValue>;
  sys: SysStats | null;
  statuses: StatusItem[];
  notifications: Notification[];
  media: MediaInfo | null;
  /** M15: the weather section (null = off or no data yet) and the OBS section (null = the integration is off). */
  weather: WeatherInfo | null;
  obs: ObsInfo | null;
  /** M16: the limits of Claude Code / Codex and the permission questions (null = both integrations are off). */
  ai: AiInfo | null;
  /** The numbers of the signed-in Twitch channel (null = Twitch is off). */
  twitch: TwitchInfo | null;
  /** The numbers of the signed-in YouTube channel (null = YouTube is off). */
  youtube: YoutubeInfo | null;
  /** M17: the charge of the PC and Bluetooth devices, and unread mail (absent or null = nothing to show). */
  battery?: BatteryInfo | null;
  mail?: MailInfo | null;
  /** M22 (DATA_MODEL.md 13.7): the next reminders and the lists (null = nothing yet). */
  reminders?: RemindersSection | null;
  lists?: ListsSection | null;
  /** M24 (DATA_MODEL.md 14): synced/plain lyrics of current track (null = off or none). */
  lyrics?: LyricsInfo | null;
  /** The pages that follow an app (`apps` set) and the app in front on the PC: what the `context` widget shows. */
  pages?: Page[];
  focusApp?: string | null;
  /** `img:` pictures by hash (the `icons` section). */
  icons: Record<string, string>;
  /** `display.keyFace` / `display.keyLabels` (DESIGN.md 2.1): the look keys have unless a key says otherwise. */
  keyFace?: "standard" | "full";
  keyLabels?: boolean;
}

export const EMPTY_DATA: DeckData = { online: false, vars: {}, sys: null, statuses: [], notifications: [], media: null, weather: null, obs: null, ai: null, twitch: null, youtube: null, icons: {} };

/** What the deck asks its host to do. */
export interface DeckCallbacks {
  /** Press a key: resolves on `ok`, rejects with `{ code, message? }`. */
  onPress: (actionId: string) => Promise<void>;
  /** A key that failed (the host shows it, for example as a toast). */
  onPressError: (e: { code: string; message?: string }) => void;
  /** A `navigate` key: open the page with this id, or `"$back"`: the page the user came from (DATA_MODEL.md 3.8). */
  onNavigate: (pageId: string) => void;
  /** A notification row was tapped. */
  onDismiss: (notifId: string) => void;
  /** A notification of Windows that can be opened was tapped: the PC opens its program (DATA_MODEL.md 10.4). */
  onOpenNotification?: (notifId: string) => void;
  /** A key with `popup` wants its group opened; `anchor` is the key's box on screen. */
  onOpenPopup?: (popupId: string, anchor: { x: number; y: number; width: number; height: number }) => void;
  /** Tactile feedback moments inside the deck: a key was touched, a hold-to-confirm completed. */
  onFeedback?: (moment: "touch" | "confirm") => void;
  /** The timer widget ran down: the host shows a toast and wakes the screen (DATA_MODEL.md 6.7). */
  onTimerDone?: () => void;
  /** The badge of the `ai` widget was tapped: show the permission sheet again (DATA_MODEL.md 6.8). */
  onOpenPerm?: () => void;
  /** The `ai` widget was tapped: show the list of answered questions. */
  onOpenPermHistory?: () => void;
  /** An item of the `list` widget was tapped: the PC marks or unmarks it (`list_toggle`). */
  onToggleListItem?: (listId: string, itemId: string) => void;
  /** M24: user tapped `-0.5 s` / `+0.5 s` in lyrics overlay (`lyrics_offset`). */
  onLyricsOffset?: (trackKey: string, deltaMs: -500 | 500) => void;
}

export type DeckEnv = DeckData & DeckCallbacks;

const NOOP_ASYNC = () => Promise.resolve();
const NOOP = () => {};

export const DeckEnvContext = createContext<DeckEnv>({ ...EMPTY_DATA, onPress: NOOP_ASYNC, onPressError: NOOP, onNavigate: NOOP, onDismiss: NOOP });

export function useDeckEnv(): DeckEnv {
  return useContext(DeckEnvContext);
}
