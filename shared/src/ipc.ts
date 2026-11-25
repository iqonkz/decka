import type { Id, WeatherInfo } from "./protocol";
import type { ActionsFile } from "./actions";
import type { LayoutFile } from "./layout";
import type { ConfigFile, DeviceInfo } from "./config";
import type { PageTemplate } from "./templates";
import type { IslandRect } from "./island";

/** One line of the connection check (`diag.rs`); `id` and `status` pick the sentence, `detail` fills it. */
export interface DiagCheck {
  id: "server" | "bind" | "address" | "reach" | "firewall" | "clients";
  status: "ok" | "warn" | "fail" | "unknown";
  detail: string;
}

export interface AgentStatus {
  version: string;
  focusApp: string | null;                            // M11: the active app on the PC (Status tab)
  listening: { bind: string; port: number } | null;   // null = port busy / server did not start
  listenError?: string;
  listenBusy?: { port: number; app?: string };         // T21.16: another program holds the port
  lanIps: string[];
  clients: { deviceId: Id; name: string; remote: string; connectedAt: number }[];
  usb: { enabled: boolean; adbFound: boolean; devices: { serial: string; reversed: boolean }[]; lastError?: string };
  configDir: string;
}
/** A Start menu shortcut to a program (M12, DATA_MODEL.md 8.3). */
export interface InstalledApp {
  name: string;          // shortcut name without .lnk
  program: string;       // full path of the .exe
  args: string[];        // shortcut arguments, split by the Windows command-line rules
  cwd?: string;          // working folder of the shortcut
  icon?: string;         // 32 px data URL for the list (may be absent)
}
/** M14, ARCHITECTURE.md 8a: the answer of the last check for a newer release. */
export interface UpdateStatus {
  available: { version: string; url: string; publishedAt: string } | null;  // a newer release, when the last check found one
  checkedAt: number | null;                                                   // ms since the epoch; null before the first check
  failed: boolean;                                                            // the last check could not reach GitHub
}
export interface PairingInfo { code: string; url: string; qrSvg: string; expiresAt: number; }
export interface ValidationIssue { path: string; message: string; }   // path = JSON pointer
export type SaveResult = { ok: true } | { ok: false; issues: ValidationIssue[] };

/** M16: Claude Code and Codex as the settings window sees them (PROTOCOL.md 9). */
export interface CcStatus {
  installed: boolean;              // our status line and hook are in ~/.claude/settings.json
  statusLineConflict: boolean;     // the status line belongs to another tool: only approvals work
  lastSeenAt?: number;             // the last status line or hook call
  codexFound: boolean;             // %USERPROFILE%\.codex\sessions exists
  codexHook: boolean;              // our hook is in %USERPROFILE%\.codex\hooks.json (Codex asks to trust it with /hooks)
}

/** Twitch as the settings window sees it. */
export interface TwitchStatus {
  enabled: boolean;
  signedIn: boolean;
  login?: string;
  error?: string;                  // of the sign-in, or of the last look at the channel
  pending?: { userCode: string; verificationUri: string; expiresAt: number };   // a sign-in in progress: type the code on twitch.tv/activate
}

/** YouTube as the settings window sees it. */
/** `access`: `allowed`, `denied` (blocked in Windows Settings → Privacy → Notifications) or `unknown`. */
/** M18 (DATA_MODEL.md 11.2): the kind of network the phones use now and the switch `Allow phones on public networks`. */
export interface NetworkStatus { kind: "usb" | "private" | "public"; allowPublic: boolean; }
export interface WindowsNotificationsStatus { enabled: boolean; access: "allowed" | "denied" | "unknown"; apps: string[]; }

export interface YoutubeStatus {
  enabled: boolean;
  signedIn: boolean;
  secretSet: boolean;              // the client secret is stored (its value never leaves the agent)
  channel?: string;
  error?: string;                  // of the sign-in, or of the last look at the channel
  pending?: { userCode: string; verificationUri: string; expiresAt: number };   // a sign-in in progress: type the code on google.com/device
}

/** M21: the voice package as the settings window sees it (DATA_MODEL.md 12.7). Window event `voice-status` carries it on every change. */
export interface VoiceStatus {
  state: "off" | "downloading" | "ready" | "loading" | "error";
  progress?: { done: number; total: number };
  hardware: { gpu?: string; vramGb?: number; ramGb: number };
  model: "standard" | "light";
  downloadGb: number;
  hotkey: string[];
  keepLoaded: boolean;
  speak: boolean;
  phoneButton: boolean;
  mic?: string;
  mics: string[];
  error?: string;
}
/** M22 (DATA_MODEL.md 13.7): the rows of the Reminders window. */
/** 0.23 (DATA_MODEL.md 13.4): a repeating reminder; absent = once. */
export type ReminderRepeat = "daily" | "weekdays" | "weekly";
export interface ReminderRow {
  id: string;
  text: string;
  at: string;           // of a series: the next time it fires
  done: boolean;
  repeat?: ReminderRepeat;
}
export interface ListRow {
  id: string;
  name: string;
  items: { id: string; text: string; done: boolean }[];
}
export interface MemoryFact {
  id: string;
  text: string;
  createdAt: number;
}
/** `dictation.json`: what starts a dictation, whether the text is tidied, and the words whisper should know (at most 500 of 60 characters). */
export type DictationTrigger = { kind: "doubleCtrl" } | { kind: "chord"; keys: string[] } | { kind: "off" };
/** `media.lyrics.pcOverlay` (DESIGN.md 4.17): the window of the words on the PC. Unknown values read as `auto`. */
export type LyricsPcOverlay = "off" | "auto" | "always";
export interface LyricsStatus {
  enabled: boolean;
  pcOverlay: LyricsPcOverlay;
}
export interface DictationSettings {
  trigger: DictationTrigger;
  mode: "clean" | "raw";
  dictionary: string[];
}

export interface VoiceSetArgs {
  model?: "standard" | "light";
  hotkey?: string[];
  keepLoaded?: boolean;
  speak?: boolean;
  phoneButton?: boolean;
  mic?: string;
}

export interface IpcCommands {
  voice_status(): VoiceStatus;
  voice_enable(): VoiceStatus;                                 // Turn on voice control / Retry
  voice_cancel(): VoiceStatus;
  voice_confirm(args: { id: string; yes: boolean }): void;     // the Yes/No of the PC pill (the pill window arrives with C2)
  voice_action(args: { kind: "copy" | "insert" | "speak" | "again" | `lang:${string}` }): void; // T26.3: a button under the shown answer; the PC decides what it does
  island_rect(args: IslandRect): void;                          // T26.4: the visible panel of the island; the rest of its window lets clicks through
  voice_stop(args: Record<string, never>): void;               // the stop button of the PC pill: end the recording now (12.6a)
  voice_remove(): VoiceStatus;
  voice_set(args: VoiceSetArgs): VoiceStatus;                  // an invalid hotkey is an error string
  youtube_status(): YoutubeStatus;
  youtube_login_start(): { userCode: string; verificationUri: string; expiresAt: number };   // needs the client id and the secret; an error is a string
  youtube_logout(): void;
  set_youtube_secret(args: { secret: string }): void;           // encrypts with DPAPI and saves; "" removes the secret
  twitch_status(): TwitchStatus;
  twitch_login_start(): { userCode: string; verificationUri: string; expiresAt: number };   // needs the client id in the config; an error is a string
  twitch_logout(): void;
  cc_status(): CcStatus;                                     // M16
  cc_install(): CcStatus;                                      // M16, DATA_MODEL.md 6.8; an error is a string
  cc_uninstall(): CcStatus;                                    // M16
  get_status(): AgentStatus;
  get_update_status(): UpdateStatus;                           // M14: the answer of the last check (ARCHITECTURE.md 8a)
  check_updates(): UpdateStatus;                               // M14: `Check now`: asks GitHub at once
  open_release(args: { url: string }): void;                   // M14: `Download`; only release pages of the project are opened
  get_config(): ConfigFile;
  save_config(args: { config: ConfigFile }): SaveResult;       // port/bind apply after restart_server
  restart_server(): AgentStatus;
  ai_remove_rule(args: { id?: string }): void;                // forgets one rule of "Always allow", or all of them without `id`
  diagnose_connection(): DiagCheck[];                          // the Phone tab's `Check connection`
  fix_firewall(): void;                                        // adds the Windows Firewall rule (UAC)
  regenerate_api_token(): { apiToken: string };
  get_actions(): ActionsFile;
  save_actions(args: { actions: ActionsFile }): SaveResult;
  get_layout(): LayoutFile;
  save_layout(args: { layout: LayoutFile }): SaveResult;
  test_action(args: { actionId: Id }): { ok: boolean; error?: string };
  start_pairing(): PairingInfo;                                // cancels the previous active code
  cancel_pairing(): void;
  list_devices(): DeviceInfo[];
  revoke_device(args: { deviceId: Id }): void;
  set_can_approve(args: { deviceId: Id; allowed: boolean }): boolean;   // M18: `Can answer Claude Code`
  send_test_notification(): void;                              // title "Decka test", level "info", source "decka"
  list_window_apps(): string[];                                // M11: exe names (lower case, unique, sorted) of processes with visible top-level windows, without decka-agent.exe
  get_templates(): PageTemplate[];                             // M11: DATA_MODEL.md 7
  add_template(args: { templateId: string }): SaveResult;      // M11: adds the template's actions and page, saves both files
  open_config_dir(): void;
  open_logs_dir(): void;                                       // opens <configDir>\logs
  list_installed_apps(): InstalledApp[];                       // M12: Start menu shortcuts, DATA_MODEL.md 8
  refresh_program_icons(): { updated: number; failed: number };  // M14: DATA_MODEL.md 8.1; saves the layout when updated > 0
  count_lowres_icons(): number;                                  // M14: program buttons with a PNG icon < 256 px (the Sharpen line of Look)
  describe_program(args: { path: string }): InstalledApp | { error: string };  // M14: .exe / .lnk -> program (name, target, arguments, icon)
  icon_from_program(args: { path: string }): { icon: string } | { error: string };  // M12: icon of an exe -> "img:<hash16>"
  icon_from_image(args: { path: string }): { icon: string } | { error: string };    // M12: png/jpg/ico/bmp -> "img:<hash16>"
  icon_from_svg(args: { svg: string }): { icon: string } | { error: string };         // M13: an SVG from the icon library -> "img:<hash16>" (DATA_MODEL.md 8.4)
  is_first_run(): boolean;                                     // M13: nothing paired and the layout is untouched (DESIGN.md 4.4)
  get_icon_preview(args: { icon: string }): string | null;     // M12: data URL for the settings window
  search_place(args: { query: string }): PlaceResult[] | { error: string };   // M15: Open-Meteo geocoding, up to 5 results, query 2..80 chars
  refresh_weather(): WeatherInfo | { error: string };           // M15: update now (button in the window)
  set_obs_password(args: { password: string }): void;           // M15: encrypts with DPAPI and saves; "" removes the password
  set_mail_password(args: { accountId: Id; password: string }): void; // M17: encrypts with DPAPI and saves for that mailbox; "" removes it
  lists_get(): ListRow[];                                       // M22 (DATA_MODEL.md 13.7): every list with all its items, for the editor and the Reminders window
  reminders_get(): ReminderRow[];                               // every reminder, finished ones too, soonest first
  reminder_upsert(args: { id?: string; text: string; at: string; repeat?: ReminderRepeat | "once" }): ReminderRow; // no id = new; at: local YYYY-MM-DDTHH:MM; only ids the PC issued
  reminder_delete(args: { id: string }): void;
  list_upsert(args: { id?: string; name: string; items: { id?: string; text: string; done: boolean }[] }): ListRow; // no id = new list; items replace the old ones
  list_delete(args: { id: string }): void;
  memory_get(): MemoryFact[];                                   // the facts of More -> Voice -> Memory
  memory_update(args: { id: string; text: string }): MemoryFact;
  memory_delete(args: { id: string }): void;
  memory_clear(): void;                                         // Forget all
  lyrics_get(): LyricsStatus;                                   // M24 (DATA_MODEL.md 14): the switch of the song lyrics, off by default
  lyrics_overlay_set(args: { mode: LyricsPcOverlay }): LyricsStatus; // 0.30: the lyrics window on the PC (off | auto | always)
  lyrics_set(args: { enabled: boolean }): LyricsStatus;       // on sends the artist, the title and the length of a song to lrclib.net; off clears the lyrics on the phones
  voice_dictation_get(): DictationSettings;
  voice_dictation_set(args: DictationSettings): DictationSettings; // an invalid shortcut is an error string; the answer is what was saved
  mail_check_now(): void;                                       // M17: looks at the mailboxes at once
  network_status(): NetworkStatus;                             // M18: usb / private / public and the allowPublic switch
  windows_notifications_status(): WindowsNotificationsStatus;   // M17: the switch, what Windows says about the access, the apps seen so far
  obs_status(): ObsStatus;                                      // M15: for the window (pick a scene / audio input when making keys)
}
export interface PlaceResult { name: string; country: string; region?: string; lat: number; lon: number; }
export interface ObsStatus { connected: boolean; error?: string; scenes: string[]; inputs: string[]; }
