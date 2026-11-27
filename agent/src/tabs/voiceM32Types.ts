export type WakeSensitivity = "low" | "medium" | "high";
export type WakeTestPhase = "idle" | "listening" | "hit";

export interface WakeWordConfig {
  enabled: boolean;
  sensitivity: WakeSensitivity;
  testPhase?: WakeTestPhase;
}

export interface CalendarEventItem {
  id: string;
  startTime: string;
  title: string;
  meetingProvider?: string;
  joinUrl?: string;
}

export interface CalendarConfig {
  url: string;
  connected: boolean;
  events: CalendarEventItem[];
}
