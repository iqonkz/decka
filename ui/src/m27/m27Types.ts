export interface HighlightRect {
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface HighlightEvent {
  rect: HighlightRect;
  text: string;
  step?: number;
  total?: number;
}

export interface GuideStepEvent {
  goal: string;
  step: number;
  total?: number;
  text: string;
  active: boolean;
}

export interface SavedGuideStep {
  role: string;
  name: string;
  automationId: string;
}

export interface SavedGuide {
  id: string;
  goal: string[];
  process: string;
  steps: SavedGuideStep[];
  created: string;
  uses: number;
}

export interface WorkModeApp {
  exe: string;
  monitor?: number;
  rect?: HighlightRect;
}

export interface WorkMode {
  name: string;
  apps: WorkModeApp[];
  focusMinutes?: number;
}

export type VoiceModelTier = "light" | "standard" | "9b";
