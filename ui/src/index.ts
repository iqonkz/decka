// @decka/ui: the deck itself (keys, widgets, grid) as presentational components.
// Data and callbacks come in as props; nothing here knows about sockets, stores or the settings window.
import "./deck.css";
import "./island/island.css";
import "./m27/m27.css";
import "./theme/amoled.css";
import "./theme/studio.css";
import "./theme/material.css";
import "./theme/glass.css";
import "./theme/motion.css";

export { default as Grid, type GridProps } from "./Grid";
export { default as PageDots } from "./PageDots";
export { default as Key, type KeyProps } from "./Key";
export { default as Icon } from "./Icon";
export { default as Skeleton } from "./Skeleton";
export { default as GlassLayer } from "./GlassLayer";
export { useFrost } from "./useFrost";
export { useLanguage, useApplyLanguage } from "./useLanguage";
export { frostPosition } from "./wallpaper";
export { default as PopupGrid, popupBounds, popupSize, type PopupGridProps } from "./PopupGrid";
export { DeckEnvContext, useDeckEnv, EMPTY_DATA, type DeckData, type DeckCallbacks, type DeckEnv } from "./deckEnv";
export { gridMetrics, spanSize, type GridMetrics } from "./gridMetrics";
export { DeckMetricsContext, useDeckMetrics } from "./metricsContext";
export { pressErrorTitle, resolveLook } from "./look";
export { applyTheme, resolveTheme, THEMES, WALLPAPERS, DEFAULT_ACCENT, type ResolvedTheme } from "./theme/theme";
export { colorVars, parseHex, shade, luminance } from "./theme/colors";
export { ICONS, FALLBACK_ICON } from "./icons";
export { currentPosition, formatTime } from "./widgets/mediaPos";
export { LEVEL_COLOR, STATE_COLOR, sortStatuses } from "./widgets/statusOrder";
export { relTime } from "./widgets/relTime";
export { getSysHistory, recordSys, resetSysHistory } from "./widgets/sysHistory";
export { recordAi, resetAiHistory, useAiHistory, burnKey } from "./widgets/aiHistory";
export { ProviderMark } from "./widgets/aiMarks";
export { levelColor } from "./widgets/Ai";
export { closestWindow, formatReset, windowLabel } from "./widgets/aiModel";
export { containerDensity, widgetDensityByCells, hasWidgetHeader, type WidgetDensity } from "./widgets/density";
export { averageRgbFromPixels, coverGradient, darkenCoverColor, rgbToHex, rgbToHsl, hslToRgb, sampleImageColor, COVER_MAX_LIGHTNESS } from "./widgets/coverColor";
export { effectiveMediaPositionMs, formatOffsetSeconds, selectActiveLyricIndex } from "./widgets/lyricsLogic";
export { formatRepeatLabel } from "./widgets/repeatLabel";
export { default as WidgetEmpty } from "./widgets/WidgetEmpty";
export { default as ParticlesOrb, particleCountForSize, type ParticlesOrbProps } from "./island/ParticlesOrb";
export { default as DictationOrb, type DictationOrbProps } from "./island/DictationOrb";
export { default as Island, chipIconKind, expandedStatusText, hasVoiceContent, type IslandProps, type IslandRecordingState, type IslandMemoryProcess } from "./island/Island";
export {
  orbStateFromIsland,
  type OrbState,
  type IslandMode,
  type IslandVoicePhase,
  type IslandActionKind,
  type IslandSlowProgress,
  type IslandContextChip,
  type IslandVoiceState,
  type IslandLyricsState,
  type IslandMediaKind,
  type IslandEvent,
} from "./island/islandTypes";
export { default as HighlightOverlay, computeBubblePlacement, type BubblePlacement, type BubbleSide, type HighlightOverlayProps } from "./m27/HighlightOverlay";
export { default as GuidePanel, type GuidePanelProps } from "./m27/GuidePanel";
export { default as Calendar } from "./widgets/Calendar";
export { default as TrayMiniPanel, type TrayMiniAction, type TrayMiniNextEvent, type TrayMiniPanelProps } from "./tray/TrayMiniPanel";
export type { HighlightRect, HighlightEvent, GuideStepEvent, SavedGuideStep, SavedGuide, WorkModeApp, WorkMode, VoiceModelTier } from "@decka/shared";
