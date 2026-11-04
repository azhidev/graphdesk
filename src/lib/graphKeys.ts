/** Capture-phase graph keys. WebKitGTK may report reverse Tab as ISO_Left_Tab
 * or Unidentified/Tab; do not rely on browser focus traversal or Ctrl release. */
export function graphKey(event: {
  key: string; code?: string; ctrlKey: boolean; shiftKey: boolean;
  altKey: boolean; metaKey: boolean; isComposing: boolean; keyCode?: number;
  defaultPrevented?: boolean; getModifierState?: (key: string) => boolean;
}, context: { session: boolean; open: boolean; blocked: boolean; composing: boolean }): 'next' | 'previous' | 'activate' | 'cancel' | null {
  if (!context.session || context.blocked || context.composing || event.isComposing
    || event.keyCode === 229 || event.defaultPrevented || event.getModifierState?.('AltGraph')) return null;
  const tab = event.key === 'Tab' || event.key === 'ISO_Left_Tab'
    || (event.key === 'Unidentified' && (event.code === 'Tab' || event.keyCode === 9));
  if (tab && event.ctrlKey && !event.altKey && !event.metaKey)
    return event.shiftKey || event.key === 'ISO_Left_Tab' ? 'previous' : 'next';
  if (context.open && !event.ctrlKey && !event.altKey && !event.metaKey) {
    if (event.key === 'Escape') return 'cancel';
    if (event.key === 'Enter') return 'activate';
  }
  return null;
}
