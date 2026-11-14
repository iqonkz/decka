import type { Id } from "./protocol";
import type { ActionsFile } from "./actions";
import type { LayoutFile } from "./layout";
import type { ConfigFile, DeviceInfo } from "./config";

export interface AgentStatus {
  version: string;
  listening: { bind: string; port: number } | null;   // null = port busy / server did not start
  listenError?: string;
  lanIps: string[];
  clients: { deviceId: Id; name: string; remote: string; connectedAt: number }[];
  usb: { enabled: boolean; adbFound: boolean; devices: { serial: string; reversed: boolean }[]; lastError?: string };
  configDir: string;
}
export interface PairingInfo { code: string; url: string; qrSvg: string; expiresAt: number; }
export interface ValidationIssue { path: string; message: string; }   // path = JSON pointer
export type SaveResult = { ok: true } | { ok: false; issues: ValidationIssue[] };

export interface IpcCommands {
  get_status(): AgentStatus;
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
  open_config_dir(): void;
}
