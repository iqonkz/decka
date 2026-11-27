// The sections of the "more" group, apart from their pages: the window needs the names at once, the pages when opened (T31.5).
export type MoreSection = "actions" | "integrations" | "settings" | "voice" | "screen_help" | "notes" | "reminders" | "about";

export const SECTION_IDS: MoreSection[] = ["actions", "integrations", "settings", "voice", "screen_help", "notes", "reminders", "about"];

export const isMoreSection = (s: string): s is MoreSection => (SECTION_IDS as string[]).includes(s);
