import { invoke } from '@tauri-apps/api/core';
import type { Page } from './editor/types';
export interface RecentGraph { path: string; graph_id: string }
export interface GraphRegistry { remember: boolean; reopen: boolean; entries: RecentGraph[]; warning: boolean }
export interface GraphSession { token: string; pages: Page[]; graph_id: string; path: string }
export interface GraphRequest { path: string; create: boolean; sourceSession: string | null; expectedGraph: string | null; automatic: boolean }
export interface GraphTicket extends RecentGraph { ticket: string }
export const graphApi = {
  load: () => invoke<GraphRegistry>('graph_registry_load'),
  settings: (remember: boolean, reopen: boolean) => invoke<GraphRegistry>('graph_registry_settings', { remember, reopen }),
  forget: (path: string | null) => invoke<GraphRegistry>('graph_registry_forget', { path }),
  record: (session: string) => invoke<GraphRegistry>('graph_registry_record', { session }),
  pick: (create: boolean, language: string) => invoke<string | null>('graph_pick', { create, language }),
  prepare: (request: GraphRequest) => invoke<GraphTicket>('graph_prepare', { ...request }),
  activate: (ticket: string, sourceSession: string | null) => invoke<GraphSession>('graph_activate', { ticket, sourceSession }),
  reconcile: (ticket: string) => invoke<GraphSession | null>('graph_reconcile', { ticket }),
  cancel: (ticket: string) => invoke<void>('graph_cancel', { ticket }),
};
