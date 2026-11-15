import type { Id, VarName } from "./protocol";

export interface LayoutFile {
  version: 1;
  grid: { cols: number; rows: number }; // cols 3..8, rows 2..5; default 7x4 (M10)
  display: DisplaySettings;
  pages: Page[];                  // 1..10 pages, order = swipe order
}
export interface Page {
  id: Id; name: string; items: LayoutItem[];
  apps?: string[];        // M11: context page - shown automatically while one of these apps is active on the PC (lower-case exe names)
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
}
export type ThemeId = "amoled" | "studio" | "material" | "glass";
export type WallpaperId = "aurora" | "sunset" | "ocean" | "graphite" | "album";

interface ItemBase { id: Id; x: number; y: number; w: number; h: number; }

export interface ButtonLook { label?: string; icon?: string; color?: string; }
export interface ButtonItem extends ItemBase, ButtonLook {
  kind: "button";
  actionId?: Id;          // exactly one of actionId / navigate
  navigate?: Id;          // page id
  confirm?: boolean;      // true = press by holding 800 ms
  bind?: { var: VarName; states: Record<string, ButtonLook> }; // key = String(variable value)
}
export interface ClockItem    extends ItemBase { kind: "clock"; seconds?: boolean; }
export interface SysItem      extends ItemBase { kind: "sys"; }
export interface StatusesItem extends ItemBase { kind: "statuses"; group?: string; title?: string; }
export interface StatusItemTile extends ItemBase { kind: "status"; statusId: Id; }
export interface NotificationsItem extends ItemBase { kind: "notifications"; }

export interface MediaItem extends ItemBase {   // M11: "Now playing"
  kind: "media";
  prevAction?: Id; playAction?: Id; nextAction?: Id;  // actions of the widget's keys; absent = no key
}

export type LayoutItem = ButtonItem | ClockItem | SysItem | StatusesItem | StatusItemTile | NotificationsItem | MediaItem;

// What is sent to the phone in the "layout" section: LayoutFile without "version".
export type ClientLayout = Omit<LayoutFile, "version">;
