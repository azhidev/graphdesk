export interface Page { id: string; title: string }
export interface Block {
  id: string; page_id: string; parent_id: string | null;
  position: number; content: string; task_status: string | null;
}
export type EditAction =
  | { kind: 'append' }
  | { kind: 'save'; id: string; expected: string; content: string }
  | { kind: 'split'; id: string; start: number; end: number }
  | { kind: 'indent' | 'outdent' | 'remove_empty'; id: string }
  | { kind: 'reorder'; id: string; direction: number };
export interface Outline { blocks: Block[]; can_undo: boolean; can_redo: boolean }
export interface EditResult {
  changed: Block[]; removed: string[]; focus_id: string | null;
  can_undo: boolean; can_redo: boolean;
}
export interface EditorTransport {
  page(session: string, pageId: string): Promise<Outline>;
  apply(session: string, pageId: string, action: EditAction, group: string | null): Promise<EditResult>;
  history(session: string, pageId: string, redo: boolean): Promise<EditResult>;
}
