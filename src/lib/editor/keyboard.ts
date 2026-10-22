export interface KeyContext {
  key: string; shiftKey: boolean; ctrlKey: boolean; metaKey: boolean; altKey: boolean;
  isComposing: boolean; keyCode?: number; code?: string; start: number; end: number; length: number;
  nested: boolean;
}
export type Intent = 'split' | 'indent' | 'outdent' | 'up' | 'down' | 'reorder_up' | 'reorder_down' | 'remove_empty' | 'undo' | 'redo' | 'escape';

/** Text movement, selection, bidi and composition are left to the native input. */
export function keyIntent(e: KeyContext): Intent | null {
  if (e.isComposing || e.keyCode === 229) return null;
  if ((e.ctrlKey || e.metaKey) && !e.altKey) {
    if (e.key.toLowerCase() === 'z') return e.shiftKey ? 'redo' : 'undo';
    if (e.key.toLowerCase() === 'y') return 'redo';
    return null;
  }
  if (e.altKey) {
    if (e.shiftKey && e.key === 'ArrowUp') return 'reorder_up';
    if (e.shiftKey && e.key === 'ArrowDown') return 'reorder_down';
    return null;
  }
  if (e.key === 'Escape') return 'escape';
  // Observed on Linux WebKit: Shift+Tab has key=Unidentified, code=Tab, keyCode=9.
  // Do not reinterpret unidentified text/IME keys without a known Tab code.
  if (e.key === 'Unidentified' && (e.code === 'Tab' || e.keyCode === 9)) return e.shiftKey ? 'outdent' : 'indent';
  if (e.key === 'Tab') return e.shiftKey ? 'outdent' : 'indent';
  if (e.key === 'Enter' && !e.shiftKey) return 'split';
  if (e.shiftKey || e.start !== e.end) return null;
  if (e.key === 'ArrowUp' && e.start === 0) return 'up';
  if (e.key === 'ArrowDown' && e.end === e.length) return 'down';
  if (e.key === 'Backspace' && e.start === 0) {
    if (e.length) return 'up';
    return e.nested ? 'outdent' : 'remove_empty';
  }
  return null;
}
