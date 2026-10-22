import { invoke } from '@tauri-apps/api/core';
import type { EditorTransport, Page } from './editor/types';
export type { Page, Block } from './editor/types';

export const api = {
  open: (path: string, create: boolean) => invoke<{ token: string; pages: Page[] }>('editor_open', { path, create }),
  createPage: (session: string, title: string) => invoke<Page>('editor_create_page', { session, title }),
};

export const editorApi: EditorTransport = {
  page: (session, pageId) => invoke('editor_page', { session, pageId }),
  apply: (session, pageId, action, group) => invoke('editor_apply', { session, pageId, action, group }),
  history: (session, pageId, redo) => invoke('editor_history', { session, pageId, redo }),
};
