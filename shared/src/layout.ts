import type { Id, VarName } from "./protocol";
import type { Language } from "./languages";

export interface LayoutFile {
  version: 1;
  grid: { cols: number; rows: number }; // cols 3..8, rows 2..5; default 7x4 (M10)
  display: DisplaySettings;
  pages: Page[];                  // 1..10 pages, order = swipe order
}
export interface Page {
  id: Id; name: string; items: LayoutItem[];
  apps?: string[];        // M11: context page - shown automatically while one of these apps is active on the PC (lower-case exe names)
  popup?: boolean;        // M13: a popup group - opens over the current page from a button with `popup`; not part of the swipe (DESIGN.md 4.1)
  time?: { from: string; to: string };  // shown automatically between these times of day ("HH:MM", the phone's clock; from > to crosses midnight) when no app page applies
  stayOpen?: boolean;     // M13: popup pages only: keep the panel open after a key inside it was pressed
}

export interface DisplaySettings {
  dimAfterSec: number;    // 0 = never dim; otherwise 10..3600. Default 60
  dimLevel: number;       // 0.05..1 brightness fraction when dimmed. Default 0.3
  sleepAfterSec: number;  // 0 = no screensaver; otherwise 30..86400 and > dimAfterSec. Default 600
  pixelShift: boolean;    // default true
  night: { enabled: boolean; from: string; to: string }; // "HH:MM", phone local time
  contextPages?: boolean; // M11: switch to context pages automatically; absent = true
  theme?: ThemeId;        // M13: DESIGN.md 1; absent = "amoled"; new installs get "studio"
  accent?: string;        // M13: "#rrggbb", accent of the Material theme and of the settings window; absent = "#7c5cff"
  wallpaper?: WallpaperId; // M13: Glass theme only; absent = "aurora"
  keyFace?: KeyFace;       // M14: key style by default, DESIGN.md 2.1; absent = "standard"
  keyLabels?: boolean;     // M14: show the labels of keys by default; absent = true
  language?: LanguageSetting; // M14: interface language, DESIGN.md 6; absent = "auto"
  orientation?: Orientation;  // M14: the phone's orientation, DATA_MODEL.md 3.7; absent = "landscape"
}
export type Orientation = "landscape" | "portrait";
export type LanguageSetting = "auto" | Language;
export type KeyFace = "standard" | "full";
export type ThemeId = "amoled" | "studio" | "material" | "glass";
export type WallpaperId = "aurora" | "sunset" | "ocean" | "graphite" | "album";

interface ItemBase { id: Id; x: number; y: number; w: number; h: number; }

export interface ButtonLook { label?: string; icon?: string; color?: string; }
export interface ButtonItem extends ItemBase, ButtonLook {
  kind: "button";
  actionId?: Id;          // M13: allowed combinations: actionId | navigate | popup | actionId + popup
  navigate?: Id | typeof BACK_PAGE; // id of a normal (not popup) page; M15: "$back" = the page visited before (DATA_MODEL.md 3.8)
  popup?: Id;             // M13: id of a popup page (`popup: true`)
  confirm?: boolean;      // true = press by holding 800 ms
  bind?: { var: VarName; states: Record<string, ButtonLook> }; // key = String(variable value)
  face?: KeyFace;         // M14: overrides display.keyFace for this key
  showLabel?: boolean;    // M14: overrides display.keyLabels; the label shows when it is not empty and (showLabel ?? display.keyLabels ?? true)
}
export interface ClockItem    extends ItemBase { kind: "clock"; seconds?: boolean; }
export interface SysItem      extends ItemBase { kind: "sys"; graph?: boolean; }   // M15: graph = the last 2 minutes (DATA_MODEL.md 6.5)
export interface StatusesItem extends ItemBase { kind: "statuses"; group?: string; title?: string; }
export interface StatusItemTile extends ItemBase { kind: "status"; statusId: Id; }
export interface NotificationsItem extends ItemBase { kind: "notifications"; }

export interface MediaItem extends ItemBase {   // M11: "Now playing"
  kind: "media";
  prevAction?: Id; playAction?: Id; nextAction?: Id;  // actions of the widget's keys; absent = no key
}

// M15 (DATA_MODEL.md 6.3-6.7)
export interface VolumeItem  extends ItemBase { kind: "volume"; upAction?: Id; downAction?: Id; muteAction?: Id; } // default vol-up / vol-down / vol-mute when they exist
export interface TimerItem   extends ItemBase { kind: "timer"; presets?: number[]; }   // minutes, 1..180, 1..4 values; default [25, 5]
export interface WeatherItem extends ItemBase { kind: "weather"; }
export interface ObsItem     extends ItemBase { kind: "obs"; }
/** M16 (DATA_MODEL.md 6.8): the limits of Claude Code / Codex; sizes 2x1, 2x2, 3x2, 4x2. */
export interface AiItem      extends ItemBase { kind: "ai"; show?: "both" | "claude" | "codex"; }   // default "both"
/** The Twitch numbers (live, viewers, followers); sizes 2x1..4x2. */
export interface TwitchItem  extends ItemBase { kind: "twitch"; }
/** The YouTube numbers (live, viewers, subscribers); sizes 2x1..4x2. */
export interface YoutubeItem extends ItemBase { kind: "youtube"; }
/** M17 (DATA_MODEL.md 10.6): the charge of the PC and of Bluetooth devices; sizes 1x1, 2x1, 2x2, 3x1. */
export interface BatteryItem extends ItemBase { kind: "battery"; }
/** M17 (DATA_MODEL.md 10.7): unread mail; sizes 1x1, 2x1, 2x2. */
export interface MailItem    extends ItemBase { kind: "mail"; }

/**
 * The keys of the page of the PC app in front (the page whose `apps` has it), laid out in the widget's cells in reading order.
 * Sizes 2x1 up to the whole grid; shows a hint when the app has no page.
 */
export interface ContextItem extends ItemBase { kind: "context"; }

/** M21: 1x1 voice button in the grid with a microphone. */
export interface VoiceItem extends ItemBase { kind: "voice"; label?: string; }

/** M22 (DATA_MODEL.md 13.7): the next reminders; sizes 2x1 and up, 2x2 by default. */
export interface RemindersItem extends ItemBase { kind: "reminders"; }
/** M22 (DATA_MODEL.md 13.7): one list with tick marks; `listId` is the id of a list of the PC (16 hex characters). */
export interface ListItem extends ItemBase { kind: "list"; listId?: string; }

export type LayoutItem = ButtonItem | ClockItem | SysItem | StatusesItem | StatusItemTile | NotificationsItem | MediaItem
  | VolumeItem | TimerItem | WeatherItem | ObsItem | AiItem | TwitchItem | YoutubeItem | BatteryItem | MailItem | ContextItem | VoiceItem | RemindersItem | ListItem;

/** `navigate` value of a button that goes back to the page the user came from (DATA_MODEL.md 3.8). */
export const BACK_PAGE = "$back";

// What is sent to the phone in the "layout" section: LayoutFile without "version".
export type ClientLayout = Omit<LayoutFile, "version">;
