/** Width of the content of the Keys page (the library column plus the preview) below which the library becomes a button. */
export const LIBRARY_COLLAPSE_PX = 1100;
/** The preview never gets narrower than this: the page scrolls sideways instead of squeezing it. */
export const EDITOR_MAIN_MIN_PX = 480;

/** The library sits in its own column only when the content is wide enough for it and a usable preview. */
export function libraryCollapsed(contentWidth: number): boolean {
  return contentWidth > 0 && contentWidth < LIBRARY_COLLAPSE_PX;
}
