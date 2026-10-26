import { shortcutLetter } from './editor/keyboard.ts';
export type AppShortcut = 'search' | 'capture' | 'pages' | 'open' | 'back' | 'forward';
export function appShortcut(e: { key: string; code?: string; ctrlKey: boolean; metaKey: boolean; altKey: boolean; shiftKey: boolean; isComposing: boolean; keyCode?: number; defaultPrevented?: boolean; repeat?: boolean; getModifierState?: (key: string) => boolean }, context: { modal: boolean; composing: boolean; busy: boolean; editable: boolean; session: boolean }): AppShortcut | null {
  if (context.modal || context.composing || context.busy || e.defaultPrevented || e.repeat || e.isComposing || e.keyCode === 229 || e.getModifierState?.('AltGraph')) return null;
  if (e.altKey) {
    if (e.ctrlKey || e.metaKey || e.shiftKey || context.editable || !context.session) return null;
    return e.key === 'ArrowLeft' ? 'back' : e.key === 'ArrowRight' ? 'forward' : null;
  }
  if (!(e.ctrlKey || e.metaKey)) return null;
  const key = shortcutLetter(e.key, e.code);
  if (key === 'o' && !e.shiftKey) return 'open';
  if (!context.session) return null;
  if (key === 'k' && !e.shiftKey) return 'search';
  if (key === 'p' && !e.shiftKey) return 'pages';
  if (key === 'j' && e.shiftKey) return 'capture';
  return null;
}
