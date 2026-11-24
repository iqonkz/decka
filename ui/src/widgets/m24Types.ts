import type { WeatherInfo } from "@decka/shared";

/**
 * Local PROTOCOL copies for M24 / 0.23.0 fields (Claude swaps to shared types on merge).
 * Exact shapes per docs/PROTOCOL.md: WeatherInfo.hourly, LyricsInfo, LyricsOffsetMsg, RemindersSection.
 */

export interface WeatherHourlyPoint {
  time: string;
  temp: number;
  code: number;
  isDay: boolean;
}

export interface WeatherInfoWithHourly extends WeatherInfo {
  hourly?: WeatherHourlyPoint[];
}

export interface LyricsLine {
  t?: number;
  text: string;
}

export interface LyricsInfo {
  trackKey: string;
  synced: boolean;
  lines: LyricsLine[];
  offsetMs: number;
}

export interface LyricsOffsetMsg {
  t: "lyrics_offset";
  trackKey: string;
  deltaMs: -500 | 500;
}

export type ReminderRepeat = "daily" | "weekdays" | "weekly";

export interface ReminderItemWithRepeat {
  id: string;
  text: string;
  at: string;
  repeat?: ReminderRepeat;
}

export interface RemindersSectionWithRepeat {
  items: ReminderItemWithRepeat[];
}
