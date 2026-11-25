// M27 (TASKS.md): the data the settings window and the island share with the agent - saved guides, work modes, the highlight.
// Keys and shapes follow DATA_MODEL.md; the agent writes the files, the window only shows and edits them through the IPC.

export interface HighlightRect {
  x: number;
  y: number;
  width: number;
  height: number;
}

/** The event `highlight` (agent -> highlight window): the element in CSS pixels of that window and the text of the step. */
export interface HighlightEvent {
  rect: HighlightRect;
  text: string;
  step?: number;
  total?: number;
}

/** The event `guide_step` (agent -> island): the guide in progress; `active: false` ends it. */
export interface GuideStepEvent {
  goal: string;
  step: number;
  total?: number;
  text: string;
  active: boolean;
  /** The model is thinking about this step: the estimate in seconds (shown as `~N s`). */
  etaSec?: number;
}

export interface SavedGuideStep {
  role: string;
  name: string;
  automationId: string;
}

/** `%APPDATA%/Decka/guides/<id>.json`. */
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

/** An entry of `%APPDATA%/Decka/modes.json`. */
export interface WorkMode {
  name: string;
  apps: WorkModeApp[];
  focusMinutes?: number;
}

/** The model of the assistant: light (2B), standard (4B) or extended (9B). */
export type VoiceModelTier = "light" | "standard" | "9b";
