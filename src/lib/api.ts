import { invoke } from '@tauri-apps/api/core';
import type { EditorTransport, Page, RecoveryTransport, RecoveryDraft } from './editor/types';
import type { SearchHit } from './search';
import type { Links } from './references';
import type { TaskHit, TaskFilter } from './tasks';
import type { Preferences } from './preferences';
import type { PendingNote, NoteState, NoteSubmission } from './firstNote';
export type { Page, Block } from './editor/types';

export const api = {
  open: (path: string, create: boolean) => invoke<{ token: string; pages: Page[]; graph_id: string }>('editor_open', { path, create }),
  createPage: (session: string, title: string) => invoke<Page>('editor_create_page', { session, title }),
  renamePage: (session: string, pageId: string, title: string) => invoke<Page>('editor_rename_page', { session, pageId, title }),
  journal: (session: string, date: string) => invoke<Page>('editor_journal', { session, date }),
  findJournal: (session: string, date: string) => invoke<Page | null>('editor_find_journal', { session, date }),
  tasks: (session: string, status: TaskFilter, pageId: string | null, offset: number) => invoke<TaskHit[]>('editor_tasks', { session, status, pageId, offset }),
  search: (session: string, text: string, persian = false) => invoke<SearchHit[]>('editor_search', { session, text, persian }),
  links: (session: string, pageId: string) => invoke<Links>('editor_links', { session, pageId }),
};

export const preferencesApi = {
  load: () => invoke<{ preferences: Preferences; warning: boolean }>('preferences_load'),
  save: (preferences: Preferences) => invoke<void>('preferences_save', { preferences }),
};
export const noteApi = {
  list: (session: string) => invoke<PendingNote[]>('recovery_notes', { session }),
  put: (session: string, id: string, blockId: string, date: string, content: string) => invoke<PendingNote>('recovery_note_put', { session, id, blockId, date, content }),
  remove: (session: string, id: string, version: number) => invoke<void>('recovery_note_remove', { session, id, version }),
  submit: (session: string, date: string, blockId: string, content: string) => invoke<NoteSubmission>('editor_submit_note', { session, date, blockId, content }),
  state: (session: string, blockId: string) => invoke<NoteState>('editor_note_state', { session, blockId }),
};

export const recoveryApi: RecoveryTransport & {
  list(session: string): Promise<RecoveryDraft[]>;
  remove(session: string, id: string, content: string, updatedAt: number): Promise<void>;
  inventory(): Promise<number>;
} = {
  put: (session, pageId, blockId, base, revision, content) => invoke('recovery_put', { session, pageId, blockId, base, revision, content }),
  clear: (session, blockId, content) => invoke('recovery_clear', { session, blockId, content }),
  list: session => invoke('recovery_list', { session }),
  remove: (session, id, content, updatedAt) => invoke('recovery_remove', { session, id, content, updatedAt }),
  inventory: () => invoke('recovery_inventory'),
};

export const editorApi: EditorTransport = {
  prepareDelete: (session, pageId, id) => invoke('editor_prepare_delete', { session, pageId, id }),
  page: (session, pageId) => invoke('editor_page', { session, pageId }),
  apply: (session, pageId, action, group) => invoke('editor_apply', { session, pageId, action, group }),
  history: (session, pageId, redo) => invoke('editor_history', { session, pageId, redo }),
};
