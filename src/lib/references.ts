import type { Block, Page } from './editor/types';
export interface InlineReference {
  source_block_id: string; title_key: string; label: string;
  target_page_id: string | null; target_title: string | null;
}
export interface Links { references: InlineReference[]; backlinks: Block[] }

export function completion(text: string, caret: number): { start: number; query: string } | null {
  const prefix = text.slice(0, caret);
  const start = prefix.lastIndexOf('[[');
  if (start < 0) return null;
  const query = prefix.slice(start + 2);
  return /[\[\]\r\n]/u.test(query) ? null : { start, query };
}
export function suggestions(pages: Page[], query: string): Page[] {
  const key = query.trim().normalize('NFC').toLowerCase();
  return pages.filter(p => !/[\[\]\r\n]/u.test(p.title) && p.title.normalize('NFC').toLowerCase().includes(key)).slice(0, 8);
}
export function insertReference(text: string, caret: number, title: string): { content: string; caret: number } {
  const match = completion(text, caret);
  if (!match) throw new Error('Reference completion is no longer active');
  const value = `[[${title}]]`;
  const suffix = text.slice(caret);
  const close = suffix.indexOf(']]');
  const end = close >= 0 && !/[\[\]\r\n]/u.test(suffix.slice(0, close)) ? caret + close + 2 : caret;
  return { content: text.slice(0, match.start) + value + text.slice(end), caret: match.start + value.length };
}
