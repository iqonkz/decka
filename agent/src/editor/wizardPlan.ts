import type { ActionsFile, InstalledApp, LayoutFile } from "@decka/shared";
import { findFreeSpot } from "../tabs/layoutOps";
import { addButtonAt, presetActions, type Choice } from "./addButton";

/** The ready-made actions ticked by default in step 3 of the guide (DESIGN.md 4.7). */
export const DEFAULT_PRESETS = ["play-pause", "next-track", "volume-up", "volume-down", "mute"];

export interface WizardPick {
  app: InstalledApp;
  /** `img:` icon of the program, when one could be made. */
  icon?: string;
}

/**
 * The guide only adds (DESIGN.md 4.7): the picked programs, then the ticked ready-made actions, each into the next free cell
 * of the first page in reading order. Existing buttons are never moved or removed. Whatever finds no cell is counted in `skipped`.
 */
export function planWizard(
  layout: LayoutFile,
  actions: ActionsFile,
  picks: WizardPick[],
  presetIds: string[],
): { layout: LayoutFile; actions: ActionsFile; placed: number; skipped: number } {
  const known = new Set(presetActions().map((p) => p.id));
  const choices: Choice[] = [...picks.map((p): Choice => ({ type: "app", app: p.app, ...(p.icon ? { icon: p.icon } : {}) })), ...presetIds.filter((id) => known.has(id)).map((id): Choice => ({ type: "preset", presetId: id }))];
  let lay = layout;
  let acts = actions;
  let placed = 0;
  for (const choice of choices) {
    const page = lay.pages[0];
    const spot = page ? findFreeSpot(page, lay.grid, 1, 1) : null;
    const made = spot ? addButtonAt(lay, acts, 0, spot, choice) : null;
    if (!made || !made.ok) continue;
    lay = made.layout;
    acts = made.actions;
    placed++;
  }
  return { layout: lay, actions: acts, placed, skipped: choices.length - placed };
}