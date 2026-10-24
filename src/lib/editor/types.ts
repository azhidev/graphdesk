export interface Page { id: string; title: string }
export type TaskState = 'todo' | 'doing' | 'done' | 'cancelled';
export interface Block {
  id: string; page_id: string; parent_id: string | null;
  position: number; content: string; task_status: TaskState | null;
}
export type EditAction =
  | { kind: 'append' }
  | { kind: 'save'; id: string; expected: string; content: string }
  | { kind: 'task'; id: string; expected: TaskState | null; status: TaskState | null }
  | { kind: 'split'; id: string; start: number; end: number }
  | { kind: 'indent' | 'outdent' | 'remove_empty'; id: string }
  | { kind: 'reorder'; id: string; direction: number };
export interface DeletePreview { token: string; count: number; requires_confirmation: boolean }
export type DeleteAction = { kind: 'delete'; id: string; token: string };
export interface Outline { blocks: Block[]; can_undo: boolean; can_redo: boolean }
export interface EditResult {
  changed: Block[]; removed: string[]; focus_id: string | null;
  can_undo: boolean; can_redo: boolean;
}
export interface EditorTransport {
  page(session: string, pageId: string): Promise<Outline>;
  apply(session: string, pageId: string, action: EditAction | DeleteAction, group: string | null): Promise<EditResult>;
  prepareDelete(session: string, pageId: string, id: string): Promise<DeletePreview>;
  history(session: string, pageId: string, redo: boolean): Promise<EditResult>;
}
