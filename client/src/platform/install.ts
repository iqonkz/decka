import { registerPlugin } from "@capacitor/core";

/** Own Capacitor plugin (client/android DeckaInstallPlugin): FileProvider + ACTION_VIEW of the verified APK (T20.4). */
const DeckaInstall = registerPlugin<{
  begin(): Promise<void>;
  append(options: { data: string }): Promise<void>;
  install(options: { sha256: string }): Promise<void>;
}>("DeckaInstall");

/** Bytes per bridge call (base64 of 190 KB is about 250 KB). */
const PIECE = 190 * 1024;

function toBase64(bytes: Uint8Array): string {
  let text = "";
  for (let i = 0; i < bytes.length; i += 0x8000) text += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
  return btoa(text);
}

/**
 * Hands the verified APK to Android: the plugin stores it, checks the SHA-256 once more on disk and opens the system installer.
 * The first time Android asks to allow installing from this source. The new APK has the same signature, so it installs over the
 * old one and the pairing stays.
 */
export async function installApk(bytes: Uint8Array, sha256: string): Promise<void> {
  await DeckaInstall.begin();
  for (let at = 0; at < bytes.length; at += PIECE) await DeckaInstall.append({ data: toBase64(bytes.subarray(at, at + PIECE)) });
  await DeckaInstall.install({ sha256 });
}
