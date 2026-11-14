import type { Id, VarName } from "./protocol";

export interface LayoutFile {
  version: 1;
  grid: { cols: 5; rows: 3 };     // only 5x3 is valid in v1
  display: DisplaySettings;
  pages: Page[];                  // 1..10 pages, order = swipe order
}
export interface Page { id: Id; name: string; items: LayoutItem[]; }

export interface DisplaySettings {
  dimAfterSec: number;    // 0 = never dim; otherwise 10..3600. Default 60
  dimLevel: number;       // 0.05..1 brightness fraction when dimmed. Default 0.3
  sleepAfterSec: number;  // 0 = no screensaver; otherwise 30..86400 and > dimAfterSec. Default 600
  pixelShift: boolean;    // default true
  night: { enabled: boolean; from: string; to: string }; // "HH:MM", phone local time
}

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

export type LayoutItem = ButtonItem | ClockItem | SysItem | StatusesItem | StatusItemTile | NotificationsItem;

// What is sent to the phone in the "layout" section: LayoutFile without "version".
export type ClientLayout = Omit<LayoutFile, "version">;
