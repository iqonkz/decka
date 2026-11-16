// @decka/ui: the deck itself (keys, widgets, grid) as presentational components.
// Data and callbacks come in as props; nothing here knows about sockets, stores or the settings window.
import "./deck.css";
import "./theme/amoled.css";
import "./theme/studio.css";
import "./theme/material.css";
import "./theme/glass.css";
import "./theme/motion.css";

export { default as Grid, type GridProps } from "./Grid";
export { default as PageDots } from "./PageDots";
export { default as Key, type KeyProps } from "./Key";
export { default as Icon } from "./Icon";
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
export { recordSys, resetSysHistory } from "./widgets/sysHistory";
