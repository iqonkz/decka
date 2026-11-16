import type { Id, WeatherInfo } from "./protocol";
import type { ActionsFile } from "./actions";
import type { LayoutFile } from "./layout";
import type { ConfigFile, DeviceInfo } from "./config";
import type { PageTemplate } from "./templates";

export interface AgentStatus {
  version: string;
  focusApp: string | null;                            // M11: the active app on the PC (Status tab)
  listening: { bind: string; port: number } | null;   // null = port busy / server did not start
  listenError?: string;
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

export interface IpcCommands {
  get_status(): AgentStatus;
  get_update_status(): UpdateStatus;                           // M14: the answer of the last check (ARCHITECTURE.md 8a)
  check_updates(): UpdateStatus;                               // M14: `Check now`: asks GitHub at once
  open_release(args: { url: string }): void;                   // M14: `Download`; only release pages of the project are opened
  get_config(): ConfigFile;
  save_config(args: { config: ConfigFile }): SaveResult;       // port/bind apply after restart_server
  restart_server(): AgentStatus;
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
  obs_status(): ObsStatus;                                      // M15: for the window (pick a scene / audio input when making keys)
}
export interface PlaceResult { name: string; country: string; region?: string; lat: number; lon: number; }
export interface ObsStatus { connected: boolean; error?: string; scenes: string[]; inputs: string[]; }
