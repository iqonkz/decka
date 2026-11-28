export type ReplyVoiceEngine = "piper" | "windows";

export interface PiperVoiceOption {
  id: string;
  lang: string;
  name: string;
  sizeMb: number;
}

export interface ReplyVoiceConfig {
  engine: ReplyVoiceEngine;
  voiceId: string;
  speed: number;
  downloadedIds: string[];
}

export const DEFAULT_REPLY_SPEED = 1.05;
export const MIN_REPLY_SPEED = 0.8;
export const MAX_REPLY_SPEED = 1.3;
export const REPLY_SPEED_STEP = 0.05;

export const PIPER_VOICES: PiperVoiceOption[] = [
  { id: "ru_RU-irina-medium", lang: "ru", name: "Irina (ru)", sizeMb: 63 },
  { id: "ru_RU-dmitri-medium", lang: "ru", name: "Dmitri (ru)", sizeMb: 63 },
  { id: "kk_KZ-isebai-x_low", lang: "kk", name: "Isebai (kk)", sizeMb: 28 },
  { id: "en_US-lessac-medium", lang: "en", name: "Lessac (en)", sizeMb: 63 },
  { id: "en_GB-alan-medium", lang: "en", name: "Alan (en-GB)", sizeMb: 63 },
  { id: "de_DE-thorsten-medium", lang: "de", name: "Thorsten (de)", sizeMb: 63 },
  { id: "fr_FR-siwis-medium", lang: "fr", name: "Siwis (fr)", sizeMb: 63 },
  { id: "es_ES-davefx-medium", lang: "es", name: "Davefx (es)", sizeMb: 63 },
  { id: "pt_BR-faber-medium", lang: "pt", name: "Faber (pt)", sizeMb: 63 },
  { id: "ar_JO-kareem-medium", lang: "ar", name: "Kareem (ar)", sizeMb: 63 },
  { id: "zh_CN-huayan-medium", lang: "zh", name: "Huayan (zh)", sizeMb: 63 },
  { id: "hi_IN-pratham-medium", lang: "hi", name: "Pratham (hi)", sizeMb: 63 },
];

export function clampReplySpeed(speed: number): number {
  if (!Number.isFinite(speed)) return DEFAULT_REPLY_SPEED;
  return Math.min(MAX_REPLY_SPEED, Math.max(MIN_REPLY_SPEED, Math.round(speed * 100) / 100));
}

export function findPiperVoice(id: string): PiperVoiceOption {
  return PIPER_VOICES.find((v) => v.id === id) ?? PIPER_VOICES[0]!;
}
